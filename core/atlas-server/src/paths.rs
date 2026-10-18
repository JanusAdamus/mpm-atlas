//! `GET /api/path`: parameters checked here, the search itself is atlas-paths.

use atlas_core::Error;
use atlas_paths::{Objective, PathOptions, PathQuery, PathResponse};
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use std::collections::BTreeMap;

use crate::http::{ApiError, Q, blocking, check_id, check_lang, json_bytes, to_json_vec};
use crate::{AppState, Engine};

const MAX_HOPS: u32 = 10;
const MAX_WEIGHTS: usize = 16;
const MAX_K: usize = 5;
const MAX_KNOWN: usize = 64;
/// Search budget per request: bounds the worst case so one query cannot hold a
/// blocking thread for long (the answer says `truncated` when it is hit).
const MAX_EXPANSIONS: usize = 400_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    from: String,
    to: String,
    objective: Option<String>,
    max_hops: Option<u32>,
    include_disputed: Option<bool>,
    lang: Option<String>,
    /// `evidence:1,length:0.2`: visible, adjustable weights (empty = objective defaults).
    weights: Option<String>,
    /// atlas-paths options beyond SPEC §15: number of routes, weakest conclusion
    /// worth returning, strict conjunctions, ids the reader already knows.
    k: Option<usize>,
    min_strength: Option<u8>,
    strict_conjunctions: Option<bool>,
    known: Option<String>,
}

fn parse_weights(raw: Option<&str>) -> Result<BTreeMap<String, f64>, ApiError> {
    let mut out = BTreeMap::new();
    for kv in raw.unwrap_or("").split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (k, v) = kv.split_once(':').ok_or_else(|| ApiError::bad_request(format!("weight {kv:?} is not name:value")))?;
        let ok_name = !k.is_empty() && k.len() <= 32 && k.bytes().all(|b| b.is_ascii_lowercase() || b == b'_');
        let v: f64 = v.parse().map_err(|_| ApiError::bad_request(format!("weight {k}: {v:?} is not a number")))?;
        if !ok_name || !v.is_finite() || !(0.0..=100.0).contains(&v) {
            return Err(ApiError::bad_request(format!("weight {kv:?}: name [a-z_]{{1,32}}, value in 0..=100")));
        }
        out.insert(k.to_string(), v);
    }
    if out.len() > MAX_WEIGHTS {
        return Err(ApiError::bad_request(format!("at most {MAX_WEIGHTS} weights")));
    }
    Ok(out)
}

fn parse_options(p: &Params) -> Result<PathOptions, ApiError> {
    let mut o = PathOptions { max_expansions: MAX_EXPANSIONS, ..PathOptions::default() };
    if let Some(k) = p.k {
        if !(1..=MAX_K).contains(&k) {
            return Err(ApiError::bad_request(format!("k must be in 1..={MAX_K}")));
        }
        o.k = k;
    }
    if let Some(m) = p.min_strength {
        if m > 5 {
            return Err(ApiError::bad_request("min_strength must be in 0..=5"));
        }
        o.min_strength = m;
    }
    o.strict_conjunctions = p.strict_conjunctions.unwrap_or(false);
    let known: Vec<String> =
        p.known.as_deref().unwrap_or("").split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    if known.len() > MAX_KNOWN {
        return Err(ApiError::bad_request(format!("at most {MAX_KNOWN} known ids")));
    }
    known.iter().try_for_each(|id| check_id(id))?;
    o.known = known;
    Ok(o)
}

pub fn parse(p: Params) -> Result<(PathQuery, PathOptions), ApiError> {
    check_id(&p.from)?;
    check_id(&p.to)?;
    let opts = parse_options(&p)?;
    let objective: Objective = serde_json::from_value(serde_json::json!(p.objective.as_deref().unwrap_or("best_supported")))
        .map_err(|e| ApiError::bad_request(format!("objective: {e}")))?;
    let max_hops = p.max_hops.unwrap_or(6);
    if !(1..=MAX_HOPS).contains(&max_hops) {
        return Err(ApiError::bad_request(format!("max_hops must be in 1..={MAX_HOPS}")));
    }
    let lang = p.lang.unwrap_or_else(|| "es".into());
    check_lang(&lang)?;
    let q = PathQuery {
        from: p.from,
        to: p.to,
        objective,
        max_hops,
        include_disputed: p.include_disputed.unwrap_or(false),
        lang,
        weights: parse_weights(p.weights.as_deref())?,
    };
    Ok((q, opts))
}

pub async fn handler(State(st): State<AppState>, Q(p): Q<Params>) -> Result<Response, ApiError> {
    let (q, opts) = parse(p)?;
    let body = blocking(&st.engine, move |e| to_json_vec(&e.path(&q, &opts)?)).await?;
    Ok(json_bytes(body))
}

impl Engine {
    pub fn path(&self, q: &PathQuery, opts: &PathOptions) -> atlas_core::Result<PathResponse> {
        for id in [&q.from, &q.to] {
            self.snap.resolve(id).ok_or_else(|| Error::NotFound(format!("entity {id}")))?;
        }
        let engine = self.paths.as_ref().ok_or(Error::NotImplemented("paths: no composition table loaded"))?;
        engine.find_with(q, opts)
    }
}
