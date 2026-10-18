//! `GET /api/neighborhood`: breadth-first expansion over the typed CSR, both
//! directions, with relation/state filters and a node limit.  Everything left out
//! (by limit or filter) is counted per relation in `hidden`, so aggregation keeps
//! the type and amount of what is not drawn.  Strings are borrowed from the mmap.

use atlas_core::{CURRENT_STATES_MASK, Dir, EdgeRef, EntityType, Error, EvidenceState, NodeIx, RelationType};
use axum::extract::State;
use axum::response::Response;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

use crate::http::{ApiError, Q, blocking, check_id, check_lang, check_limit, json_bytes, parse_list, to_json_vec};
use crate::{AppState, Engine};

const MAX_DEPTH: u32 = 3;
const MAX_NODES: usize = 5000;
/// Edges drawn per node allowed; beyond it they are counted in `hidden`.
const EDGES_PER_NODE: usize = 8;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    id: String,
    depth: Option<u32>,
    relations: Option<String>,
    states: Option<String>,
    limit: Option<usize>,
    lang: Option<String>,
}

pub struct Query {
    pub id: String,
    pub depth: u32,
    pub relations: Vec<RelationType>,
    pub states: Vec<EvidenceState>,
    pub limit: usize,
    pub lang: String,
}

pub fn parse(p: Params) -> Result<Query, ApiError> {
    check_id(&p.id)?;
    let depth = p.depth.unwrap_or(1);
    if !(1..=MAX_DEPTH).contains(&depth) {
        return Err(ApiError::bad_request(format!("depth must be in 1..={MAX_DEPTH}")));
    }
    let states = parse_list::<EvidenceState>(p.states.as_deref(), "states")?;
    if let Some(s) = states.iter().find(|s| !s.is_current()) {
        return Err(ApiError::bad_request(format!("state {s} is not current; the graph holds current claims only")));
    }
    let lang = p.lang.unwrap_or_else(|| "es".into());
    check_lang(&lang)?;
    Ok(Query {
        id: p.id,
        depth,
        relations: parse_list(p.relations.as_deref(), "relations")?,
        states,
        limit: check_limit(p.limit, 200, MAX_NODES)?,
        lang,
    })
}

pub async fn handler(State(st): State<AppState>, Q(p): Q<Params>) -> Result<Response, ApiError> {
    let q = parse(p)?;
    let body = blocking(&st.engine, move |e| to_json_vec(&e.neighborhood(&q)?)).await?;
    Ok(json_bytes(body))
}

#[derive(Serialize)]
pub struct Node<'a> {
    pub id: &'a str,
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    pub label: &'a str,
    pub coverage: &'static str,
    pub degree: usize,
}

/// `atlas.model.Edge`.
#[derive(Serialize)]
pub struct Edge<'a> {
    pub claim_id: &'a str,
    pub relation: RelationType,
    pub source: &'a str,
    pub target: &'a str,
    pub evidence_state: EvidenceState,
    pub conjunct_count: usize,
    pub alternative_group: Option<&'a str>,
}

#[derive(Serialize)]
pub struct Neighborhood<'a> {
    pub center: &'a str,
    pub depth: u32,
    pub limit: usize,
    pub truncated: bool,
    pub nodes: Vec<Node<'a>>,
    pub edges: Vec<Edge<'a>>,
    pub hidden: BTreeMap<RelationType, u64>,
}

fn mask<T: Copy>(items: &[T], bit: fn(T) -> u32, all: u32) -> u32 {
    if items.is_empty() { all } else { items.iter().fold(0, |m, x| m | bit(*x)) }
}

impl Engine {
    pub fn neighborhood(&self, q: &Query) -> atlas_core::Result<Neighborhood<'static>> {
        let snap = self.snap;
        let center = snap.resolve(&q.id).ok_or_else(|| Error::NotFound(format!("entity {}", q.id)))?;
        let rel_mask = mask(&q.relations, RelationType::bit, u32::MAX);
        let state_mask = mask(&q.states, EvidenceState::bit, CURRENT_STATES_MASK);
        let pass = |e: &EdgeRef| rel_mask & e.relation.bit() != 0 && state_mask & e.state.bit() != 0;
        let (out, inn) = (snap.csr(Dir::Out), snap.csr(Dir::In));

        let mut index: HashSet<NodeIx> = HashSet::from([center]);
        let mut order = vec![center];
        let mut frontier = vec![center];
        let mut truncated = false;
        for _ in 0..q.depth {
            let mut next = Vec::new();
            for &n in &frontier {
                for e in out.edges(n).iter().chain(inn.edges(n).iter()).filter(|e| pass(e)) {
                    if index.contains(&e.nbr) {
                        continue;
                    }
                    if order.len() >= q.limit {
                        truncated = true;
                        break;
                    }
                    index.insert(e.nbr);
                    order.push(e.nbr);
                    next.push(e.nbr);
                }
            }
            frontier = next;
        }

        let max_edges = q.limit * EDGES_PER_NODE;
        let mut edges = Vec::new();
        let mut hidden: BTreeMap<RelationType, u64> = BTreeMap::new();
        for &n in &order {
            for e in out.edges(n).iter() {
                if pass(&e) && index.contains(&e.nbr) && edges.len() < max_edges {
                    let c = e.claim;
                    edges.push(Edge {
                        claim_id: snap.claim_id(c),
                        relation: e.relation,
                        source: snap.node_id(n),
                        target: snap.node_id(e.nbr),
                        evidence_state: e.state,
                        conjunct_count: snap.conjunct_count(c),
                        alternative_group: snap.claim_group(c).map(|g| snap.group_id(g)),
                    });
                } else {
                    *hidden.entry(e.relation).or_default() += 1;
                }
            }
            for e in inn.edges(n).iter().filter(|e| !index.contains(&e.nbr)) {
                *hidden.entry(e.relation).or_default() += 1;
            }
        }
        truncated |= edges.len() >= max_edges;

        let nodes = order
            .iter()
            .map(|&n| Node {
                id: snap.node_id(n),
                entity_type: snap.node_type(n),
                label: snap.label(n, &q.lang),
                coverage: if snap.evidence(n).is_unknown() { "unknown" } else { "known" },
                degree: out.degree(n) + inn.degree(n),
            })
            .collect();
        Ok(Neighborhood {
            center: snap.node_id(center),
            depth: q.depth,
            limit: q.limit,
            truncated,
            nodes,
            edges,
            hidden,
        })
    }
}
