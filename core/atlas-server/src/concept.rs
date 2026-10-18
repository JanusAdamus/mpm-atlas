//! The concept layer (docs/plans/2026-10-17-capa-conceptual.md):
//!
//! - `GET /api/concept?id=&lang=&offset=&limit=&edge_limit=`: a concept, its member
//!   declarations (paged; primary first) with the evidence of each `member_of_concept`
//!   claim, and its aggregated edges to and from other concepts.
//! - `GET /api/concept/edges?from=&to=&relation=&lang=&offset=&limit=`: drill-down of
//!   one aggregated edge into the declaration-level edges behind it.
//!
//! Identity, membership and edges come from the snapshot; the evidence of each
//! membership (signal, heuristic score, provenance) from the registry when loaded.
//! The heuristic score is returned with its calibration text and is not a probability.

use atlas_core::{Error, EvidenceState, NodeIx, RelationType, Snapshot};
use axum::extract::State;
use axum::response::Response;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};

use crate::http::{ApiError, Q, blocking, check_id, check_lang, check_limit, json_bytes, to_json_vec};
use crate::{AppState, Engine, registry};

const MAX_LIMIT: usize = 200;
const MAX_OFFSET: usize = 1_000_000;
/// Declaration-level edges scanned by one drill-down before it stops counting.
const MAX_SCAN: usize = 2_000_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    id: String,
    lang: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
    edge_limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeParams {
    from: String,
    to: String,
    relation: Option<String>,
    lang: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

fn check_offset(o: Option<usize>) -> Result<usize, ApiError> {
    match o.unwrap_or(0) {
        o if o <= MAX_OFFSET => Ok(o),
        o => Err(ApiError::bad_request(format!("offset must be <= {MAX_OFFSET}, got {o}"))),
    }
}

fn lang_of(l: Option<String>) -> Result<String, ApiError> {
    let l = l.unwrap_or_else(|| "es".into());
    check_lang(&l)?;
    Ok(l)
}

pub async fn handler(State(st): State<AppState>, Q(p): Q<Params>) -> Result<Response, ApiError> {
    check_id(&p.id)?;
    let lang = lang_of(p.lang)?;
    let offset = check_offset(p.offset)?;
    let limit = check_limit(p.limit, 50, MAX_LIMIT)?;
    let edge_limit = check_limit(p.edge_limit, 20, MAX_LIMIT)?;
    let body =
        blocking(&st.engine, move |e| to_json_vec(&e.concept(&p.id, &lang, offset, limit, edge_limit)?)).await?;
    Ok(json_bytes(body))
}

pub async fn edges(State(st): State<AppState>, Q(p): Q<EdgeParams>) -> Result<Response, ApiError> {
    check_id(&p.from)?;
    check_id(&p.to)?;
    let relation = match p.relation.as_deref() {
        None | Some("") => None,
        Some(r) => Some(r.parse::<RelationType>().map_err(|e| ApiError::bad_request(format!("relation: {e}")))?),
    };
    let lang = lang_of(p.lang)?;
    let offset = check_offset(p.offset)?;
    let limit = check_limit(p.limit, 50, MAX_LIMIT)?;
    let body = blocking(&st.engine, move |e| {
        to_json_vec(&e.concept_edges(&p.from, &p.to, relation, &lang, offset, limit)?)
    })
    .await?;
    Ok(json_bytes(body))
}

fn concept_node(snap: &Snapshot, id: &str) -> atlas_core::Result<NodeIx> {
    let n = snap.resolve(id).ok_or_else(|| Error::NotFound(format!("entity {id}")))?;
    snap.concept_index(n).ok_or_else(|| Error::NotFound(format!("{id} is not a concept (no member declarations)")))?;
    Ok(n)
}

fn node_ref(snap: &Snapshot, n: NodeIx, lang: &str) -> Value {
    json!({
        "id": snap.node_id(n),
        "label": snap.label(n, lang),
        "type": snap.node_type(n),
        "derived": snap.is_derived_concept(n),
    })
}

/// Aggregated edge as JSON: the other concept, the total and the (relation, state) counts.
fn edge_json(snap: &Snapshot, other: NodeIx, e: &atlas_core::ConceptEdge<'_>, lang: &str) -> Value {
    let stats: Vec<Value> = e
        .stats()
        .map(|s| {
            json!({
                "relation": s.relation, "evidence_state": s.state, "count": s.count,
                "example_claim": snap.claim_id(s.example),
            })
        })
        .collect();
    json!({"concept": node_ref(snap, other, lang), "total": e.total(), "stats": stats})
}

/// Membership evidence from the registry: claim id -> (signal, evidence, rank, score, provenance).
fn membership_evidence(db: &std::path::Path, ids: &[&str]) -> atlas_core::Result<HashMap<String, Value>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    registry::with(db, |c| {
        let marks = vec!["?"; ids.len()].join(",");
        let mut st = c.prepare(&format!("SELECT id, data FROM claims WHERE id IN ({marks})"))?;
        let rows = st.query_map(rusqlite::params_from_iter(ids), |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        let mut out = HashMap::new();
        for row in rows {
            let (id, data) = row?;
            let v: Value = serde_json::from_str(&data).map_err(|e| Error::Registry(format!("stored JSON: {e}")))?;
            let prov: Vec<Value> = v["provenance"].as_array().cloned().unwrap_or_default();
            // The projection writes {signal, evidence, rank} as the quote (atlas/concepts.py).
            let quote: Value = prov
                .iter()
                .find_map(|p| p["quote"].as_str())
                .and_then(|q| serde_json::from_str(q).ok())
                .unwrap_or(Value::Null);
            let provenance: Vec<Value> = prov
                .iter()
                .map(|p| {
                    json!({
                        "source_id": p["source_id"], "locator": p["locator"], "extractor": p["extractor"],
                        "extractor_version": p["extractor_version"], "license": p["license"],
                    })
                })
                .collect();
            out.insert(
                id,
                json!({
                    "signal": quote.get("signal").cloned().unwrap_or(Value::Null),
                    "evidence": quote.get("evidence").cloned().unwrap_or(Value::Null),
                    "rank": quote.get("rank").cloned().unwrap_or(Value::Null),
                    "score": v.get("confidence").cloned().unwrap_or(Value::Null),
                    "provenance": provenance,
                }),
            );
        }
        Ok(out)
    })
}

fn description(db: &std::path::Path, id: &str, lang: &str) -> atlas_core::Result<Option<String>> {
    registry::with(db, |c| {
        let data: Option<String> = match c.query_row("SELECT data FROM entities WHERE id = ?1", [id], |r| r.get(0)) {
            Ok(d) => Some(d),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };
        let v: Value = data.and_then(|d| serde_json::from_str(&d).ok()).unwrap_or(Value::Null);
        let d = &v["descriptions"];
        Ok(d[lang].as_str().or_else(|| d["en"].as_str()).or_else(|| d["es"].as_str()).map(String::from))
    })
}

impl Engine {
    pub fn concept(&self, id: &str, lang: &str, offset: usize, limit: usize, edge_limit: usize) -> atlas_core::Result<Value> {
        let snap = self.snap;
        let c = concept_node(snap, id)?;
        // Memberships: primary first, then by evidence state, then label (stable paging).
        let ms = snap.concept_memberships(c);
        let mut rows: Vec<(bool, EvidenceState, &str, NodeIx, u32)> = ms
            .iter()
            .map(|e| (snap.primary_concept(e.nbr) == Some(c), e.state, snap.label(e.nbr, lang), e.nbr, e.claim))
            .collect();
        rows.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(b.2)).then(a.3.cmp(&b.3)));
        let mut states: BTreeMap<EvidenceState, usize> = BTreeMap::new();
        for r in &rows {
            *states.entry(r.1).or_default() += 1;
        }
        let page: Vec<_> = rows.iter().skip(offset).take(limit).collect();
        let claim_ids: Vec<&str> = page.iter().map(|r| snap.claim_id(r.4)).collect();
        let (evidence, description) = match self.db.as_deref() {
            Some(db) => (membership_evidence(db, &claim_ids)?, description(db, snap.node_id(c), lang)?),
            None => (HashMap::new(), None),
        };
        let members: Vec<Value> = page
            .iter()
            .map(|&&(primary, state, label, n, claim)| {
                let cid = snap.claim_id(claim);
                let mut m = json!({
                    "id": snap.node_id(n), "label": label, "type": snap.node_type(n), "primary": primary,
                    "claim_id": cid, "evidence_state": state,
                });
                match evidence.get(cid) {
                    Some(ev) => m.as_object_mut().expect("object").extend(ev.as_object().cloned().unwrap_or_default()),
                    None => {
                        let prov: Vec<Value> = snap
                            .claim_provenance(claim)
                            .map(|p| json!({"source_id": p.source_id, "locator": p.locator, "license": p.license}))
                            .collect();
                        m["provenance"] = json!(prov);
                    }
                }
                m
            })
            .collect();
        let mut out_edges: Vec<_> = snap.concept_edges(c).collect();
        out_edges.sort_by_key(|e| (std::cmp::Reverse(e.total()), e.target));
        let mut in_edges: Vec<(NodeIx, atlas_core::ConceptEdge<'_>)> = snap
            .concepts()
            .iter()
            .flat_map(|&s| snap.concept_edges(s).filter(|e| e.target == c).map(move |e| (s, e)))
            .collect();
        in_edges.sort_by_key(|(s, e)| (std::cmp::Reverse(e.total()), *s));
        Ok(json!({
            "concept": node_ref(snap, c, lang),
            "description": description,
            "members_total": snap.concept_members(c).len(),
            "memberships_total": rows.len(),
            "memberships_by_state": states,
            "offset": offset,
            "limit": limit,
            "truncated": offset + page.len() < rows.len(),
            "members": members,
            "edges_out_total": out_edges.len(),
            "edges_out": out_edges.iter().take(edge_limit).map(|e| edge_json(snap, e.target, e, lang)).collect::<Vec<_>>(),
            "edges_in_total": in_edges.len(),
            "edges_in": in_edges.iter().take(edge_limit).map(|(s, e)| edge_json(snap, *s, e, lang)).collect::<Vec<_>>(),
        }))
    }

    /// Declaration-level edges behind the aggregated edge `from` -> `to` (primary
    /// member -> primary member, any relation but `member_of_concept`).
    pub fn concept_edges(
        &self,
        from: &str,
        to: &str,
        relation: Option<RelationType>,
        lang: &str,
        offset: usize,
        limit: usize,
    ) -> atlas_core::Result<Value> {
        let snap = self.snap;
        let (a, b) = (concept_node(snap, from)?, concept_node(snap, to)?);
        let mut total = 0usize;
        let mut scanned = 0usize;
        let mut page = Vec::new();
        'outer: for &m in snap.concept_members(a) {
            for e in snap.out_edges(m).iter() {
                scanned += 1;
                if scanned > MAX_SCAN {
                    break 'outer;
                }
                if e.relation == RelationType::MemberOfConcept
                    || relation.is_some_and(|r| r != e.relation)
                    || snap.primary_concept(e.nbr) != Some(b)
                {
                    continue;
                }
                if total >= offset && page.len() < limit {
                    page.push(json!({
                        "claim_id": snap.claim_id(e.claim), "relation": e.relation, "evidence_state": e.state,
                        "source": node_ref(snap, m, lang), "target": node_ref(snap, e.nbr, lang),
                    }));
                }
                total += 1;
            }
        }
        Ok(json!({
            "from": node_ref(snap, a, lang),
            "to": node_ref(snap, b, lang),
            "relation": relation,
            "total": total,
            "complete": scanned <= MAX_SCAN,
            "offset": offset,
            "limit": limit,
            "edges": page,
        }))
    }
}

/// `concept` block of `/api/entity`: what the panel needs to link a node to the concept layer.
pub fn entity_concept_info(snap: &Snapshot, n: NodeIx, lang: &str) -> Value {
    let as_concept = snap.concept_index(n).map(|_| {
        json!({
            "members": snap.concept_members(n).len(),
            "memberships": snap.concept_memberships(n).len(),
            "derived": snap.is_derived_concept(n),
        })
    });
    let primary = snap.primary_concept(n).map(|c| node_ref(snap, c, lang));
    json!({"as_concept": as_concept, "primary_concept": primary})
}
