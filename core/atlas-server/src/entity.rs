//! `GET /api/entity/{id}`: the entity, its current claims (`Claim.to_json()` plus the
//! labels of every participant) and the sources they cite.  Identity, merges,
//! coverage and counts come from the snapshot; full text comes from the registry.

use atlas_core::{Dir, Error, EvidenceState, NodeIx, RelationType, Snapshot};
use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::http::{ApiError, Q, blocking, check_id, check_lang, check_limit, json_bytes, to_json_vec};
use crate::{AppState, Engine, registry};

const MAX_CLAIMS: usize = 1000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    lang: Option<String>,
    limit: Option<usize>,
}

pub async fn handler(
    State(st): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Q(p): Q<Params>,
) -> Result<Response, ApiError> {
    check_id(&id)?;
    let lang = p.lang.unwrap_or_else(|| "es".into());
    check_lang(&lang)?;
    let limit = check_limit(p.limit, 200, MAX_CLAIMS)?;
    let body = blocking(&st.engine, move |e| to_json_vec(&e.entity(&id, &lang, limit)?)).await?;
    Ok(json_bytes(body))
}

/// Fill the fields `store._compact` dropped, so the shape is exactly `to_json()`.
/// (field, default value) pairs of the fields `store._compact` may drop.
type Defaults = [(&'static str, fn() -> Value)];

fn with_defaults(v: &mut Value, defaults: &Defaults) {
    if let Value::Object(m) = v {
        for (k, d) in defaults {
            m.entry(*k).or_insert_with(d);
        }
    }
}

fn arr() -> Value {
    json!([])
}
fn obj() -> Value {
    json!({})
}
fn null() -> Value {
    Value::Null
}

const ENTITY_DEFAULTS: &Defaults = &[
    ("aliases", arr),
    ("descriptions", obj),
    ("statements", obj),
    ("formula", null),
    ("external_ids", arr),
    ("provenance", arr),
];
const CLAIM_DEFAULTS: &Defaults = &[
    ("assumptions", arr),
    ("validity_domain", null),
    ("alternative_group", null),
    ("supersedes", null),
    ("confidence", null),
    ("historical_date", null),
    ("explanation", obj),
    ("disagreements", arr),
];

fn parse(data: &str) -> atlas_core::Result<Value> {
    serde_json::from_str(data).map_err(|e| Error::Registry(format!("stored JSON: {e}")))
}

fn fill_provenance_quotes(v: &mut Value) {
    if let Some(ps) = v.get_mut("provenance").and_then(Value::as_array_mut) {
        for p in ps {
            with_defaults(p, &[("quote", null)]);
        }
    }
}

/// Current claims with any participant in `ids` (as `Store.claims_for(current_only=True)`),
/// except the `member_of_concept` claims whose object is the entity: a concept can have
/// thousands; `/api/concept` pages them and `counts_by_relation` counts them.
fn claims(c: &Connection, ids: &[&str], limit: usize) -> atlas_core::Result<(Vec<Value>, bool)> {
    let states: Vec<String> = EvidenceState::ALL
        .iter()
        .filter(|s| s.is_current())
        .map(|s| format!("'{}'", s.as_str()))
        .collect();
    let marks = vec!["?"; ids.len()].join(",");
    let sql = format!(
        "SELECT data, evidence_state, superseded_by, formal FROM claims WHERE id IN \
         (SELECT DISTINCT cp.claim_id FROM claim_participants cp JOIN claims c2 ON c2.id = cp.claim_id \
          WHERE cp.entity_id IN ({marks}) AND NOT (cp.role = 'object' AND c2.relation = 'member_of_concept')) \
         AND evidence_state IN ({}) ORDER BY id LIMIT {}",
        states.join(","),
        limit + 1
    );
    let mut st = c.prepare_cached(&sql)?;
    let rows = st.query_map(rusqlite::params_from_iter(ids), |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (data, state, sup_by, formal) = row?;
        let mut v = parse(&data)?;
        v["evidence_state"] = json!(state);
        v["superseded_by"] = json!(sup_by);
        if let Some(f) = formal {
            v["formal"] = parse(&f)?;
        }
        with_defaults(&mut v, &[("formal", null)]);
        with_defaults(&mut v, CLAIM_DEFAULTS);
        fill_provenance_quotes(&mut v);
        out.push(v);
    }
    let truncated = out.len() > limit;
    out.truncate(limit);
    Ok((out, truncated))
}

fn sources(c: &Connection, ids: &BTreeSet<String>) -> atlas_core::Result<Map<String, Value>> {
    let mut st = c.prepare_cached("SELECT data FROM sources WHERE id = ?1")?;
    let mut out = Map::new();
    for id in ids {
        if let Some(data) = st.query_row([id], |r| r.get::<_, String>(0)).map(Some).or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            e => Err(e),
        })? {
            out.insert(id.clone(), parse(&data)?);
        }
    }
    Ok(out)
}

/// Current claims per relation where the node is subject, object or premise (CSR,
/// both directions; a conjunction counts once).
fn counts_by_relation(snap: &Snapshot, n: NodeIx) -> BTreeMap<RelationType, usize> {
    let mut seen: HashSet<u32> = HashSet::new();
    let mut out = BTreeMap::new();
    for dir in [Dir::Out, Dir::In] {
        for e in snap.csr(dir).edges(n).iter() {
            if seen.insert(e.claim) {
                *out.entry(e.relation).or_insert(0) += 1;
            }
        }
    }
    out
}

/// `label` of every participant, in `lang`, resolved through merges.
fn participant_labels(snap: &Snapshot, claim: &mut Value, lang: &str) {
    let labels: Map<String, Value> = claim["participants"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| p["entity"].as_str())
        .filter_map(|id| snap.resolve(id).map(|n| (id.to_string(), json!(snap.label(n, lang)))))
        .collect();
    claim["labels"] = Value::Object(labels);
}

impl Engine {
    pub fn entity(&self, id: &str, lang: &str, limit: usize) -> atlas_core::Result<Value> {
        let snap = self.snap;
        let n = snap.resolve(id).ok_or_else(|| Error::NotFound(format!("entity {id}")))?;
        let db = self.db.as_deref().ok_or(Error::NotImplemented("entity detail needs the registry (--db)"))?;
        let canonical = snap.node_id(n);
        let merged: Vec<&str> = snap.merged_from(n).into_iter().map(|m| snap.node_id(m)).collect();
        let ids: Vec<&str> = std::iter::once(canonical).chain(merged.iter().copied()).collect();
        let (entity, mut claims, truncated, sources) = registry::with(db, |c| {
            let data: String = c
                .query_row("SELECT data FROM entities WHERE id = ?1", [canonical], |r| r.get(0))
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("entity {canonical} in registry")),
                    e => e.into(),
                })?;
            let mut entity = parse(&data)?;
            with_defaults(&mut entity, ENTITY_DEFAULTS);
            fill_provenance_quotes(&mut entity);
            let (claims, truncated) = claims(c, &ids, limit)?;
            let cited: BTreeSet<String> = claims
                .iter()
                .chain(std::iter::once(&entity))
                .flat_map(|v| v["provenance"].as_array().into_iter().flatten())
                .filter_map(|p| p["source_id"].as_str().map(String::from))
                .collect();
            Ok((entity, claims, truncated, sources(c, &cited)?))
        })?;
        for c in &mut claims {
            participant_labels(snap, c, lang);
        }
        Ok(json!({
            "entity": entity,
            "canonical_id": canonical,
            "merged_from": merged,
            "coverage": if snap.evidence(n).is_unknown() { "unknown" } else { "known" },
            "label": snap.label(n, lang),
            "counts_by_relation": counts_by_relation(snap, n),
            "limit": limit,
            "truncated": truncated,
            "claims": claims,
            "sources": sources,
            "concept": crate::concept::entity_concept_info(snap, n, lang),
        }))
    }
}
