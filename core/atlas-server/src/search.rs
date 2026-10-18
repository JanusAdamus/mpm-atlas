//! `GET /api/search`.  Uses atlas-search when its index opens; otherwise the
//! registry's FTS5 lexical lane with exactly `Store.search` semantics (accent- and
//! case-insensitive, last token a prefix, field weights, merges resolved, one hit
//! per entity), so the API answers the same shape before the hybrid index exists.

use atlas_core::{EntityType, Error, Snapshot};
use atlas_search::{Hit, LaneScore, Lanes, Matched, SearchQuery, SearchResponse};
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::http::{ApiError, Q, blocking, check_lang, check_limit, json_bytes, parse_list, to_json_vec};
use crate::{AppState, Engine, registry};

const MAX_QUERY_CHARS: usize = 256;
const MAX_LIMIT: usize = 100;
/// `Store.FIELD_WEIGHT`.
const FIELD_WEIGHT: [(&str, f64); 5] =
    [("label", 3.0), ("alias", 2.0), ("external_id", 2.0), ("description", 1.0), ("statement", 1.0)];
/// Rows read per requested hit before deduplication by entity (as `Store.search`).
const OVERFETCH: usize = 10;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    q: String,
    lang: Option<String>,
    types: Option<String>,
    limit: Option<usize>,
}

pub fn parse(p: Params) -> Result<SearchQuery, ApiError> {
    let q = p.q.trim().to_string();
    if q.is_empty() || q.chars().count() > MAX_QUERY_CHARS {
        return Err(ApiError::bad_request(format!("q must have 1..={MAX_QUERY_CHARS} characters")));
    }
    let lang = p.lang.unwrap_or_else(|| "es".into());
    check_lang(&lang)?;
    let types = parse_list::<EntityType>(p.types.as_deref(), "types")?;
    Ok(SearchQuery { q, lang, types, limit: check_limit(p.limit, 20, MAX_LIMIT)? })
}

pub async fn handler(State(st): State<AppState>, Q(p): Q<Params>) -> Result<Response, ApiError> {
    let q = parse(p)?;
    let body = blocking(&st.engine, move |e| to_json_vec(&e.search(&q)?)).await?;
    Ok(json_bytes(body))
}

impl Engine {
    pub fn search(&self, q: &SearchQuery) -> atlas_core::Result<SearchResponse> {
        match (&self.search, &self.db) {
            (Some(idx), _) => idx.search(self.snap, q),
            (None, Some(db)) => registry_search(self.snap, db, q),
            (None, None) => Err(Error::NotImplemented("search: no atlas-search index and no registry")),
        }
    }
}

/// FTS5 MATCH expression of `Store._fts_query`: `\w+` tokens, quoted, last one a prefix.
fn fts_query(q: &str) -> Option<String> {
    let toks: Vec<&str> = q.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|t| !t.is_empty()).collect();
    let (last, rest) = toks.split_last()?;
    let mut m: Vec<String> = rest.iter().map(|t| format!("\"{t}\"")).collect();
    m.push(format!("\"{last}\"*"));
    Some(m.join(" "))
}

fn weight(field: &str) -> f64 {
    FIELD_WEIGHT.iter().find(|(f, _)| *f == field).map_or(1.0, |(_, w)| *w)
}

struct Row {
    target: String,
    field: String,
    lang: String,
    text: String,
    bm25: f64,
}

fn fts_rows(db: &Path, m: &str, lang: &str, n: usize) -> atlas_core::Result<Vec<Row>> {
    registry::with(db, |c| {
        let mut st = c.prepare_cached(
            "SELECT target_id, field, lang, text, bm25(fts) FROM fts WHERE fts MATCH ?1 \
             AND lang IN (?2, 'und') ORDER BY bm25(fts) LIMIT ?3",
        )?;
        let rows = st.query_map(rusqlite::params![m, lang, n as i64], |r| {
            Ok(Row { target: r.get(0)?, field: r.get(1)?, lang: r.get(2)?, text: r.get(3)?, bm25: r.get(4)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

/// The registry's lexical lane over the snapshot's nodes (merges resolved there).
/// Entities newer than the snapshot are skipped: the snapshot defines what is served.
pub fn registry_search(snap: &Snapshot, db: &Path, q: &SearchQuery) -> atlas_core::Result<SearchResponse> {
    let mut resp =
        SearchResponse { query: q.q.clone(), lang: q.lang.clone(), limit: q.limit, truncated: false, hits: vec![] };
    let Some(m) = fts_query(&q.q) else { return Ok(resp) };
    let mut best: HashMap<u32, (f64, Row)> = HashMap::new();
    for row in fts_rows(db, &m, &q.lang, q.limit * OVERFETCH)? {
        let Some(n) = snap.resolve(&row.target) else { continue };
        if !q.types.is_empty() && !q.types.contains(&snap.node_type(n)) {
            continue;
        }
        let score = -row.bm25 * weight(&row.field);
        if best.get(&n).is_none_or(|(s, _)| score > *s) {
            best.insert(n, (score, row));
        }
    }
    let mut hits: Vec<(u32, f64, Row)> = best.into_iter().map(|(n, (s, r))| (n, s, r)).collect();
    hits.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    resp.truncated = hits.len() > q.limit;
    resp.hits = hits
        .into_iter()
        .take(q.limit)
        .enumerate()
        .map(|(i, (n, score, r))| Hit {
            id: snap.node_id(n).to_string(),
            entity_type: snap.node_type(n),
            label: snap.label(n, &q.lang).to_string(),
            matched: Matched { field: r.field, lang: r.lang, text: r.text },
            score,
            lanes: Lanes { lexical: Some(LaneScore { rank: i as u32 + 1, score }), ..Lanes::default() },
        })
        .collect();
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::fts_query;

    #[test]
    fn fts_query_quotes_tokens_and_prefixes_the_last() {
        assert_eq!(fts_query("teorema de pit").as_deref(), Some("\"teorema\" \"de\" \"pit\"*"));
        assert_eq!(fts_query("Pitágoras\"; DROP").as_deref(), Some("\"Pitágoras\" \"DROP\"*"));
        assert_eq!(fts_query(" ¿? "), None);
    }
}
