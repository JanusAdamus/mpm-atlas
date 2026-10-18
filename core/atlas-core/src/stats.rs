//! Counts over a snapshot (for `atlas stats` and `/api/coverage`-like summaries).

use rayon::prelude::*;
use serde::Serialize;
use std::collections::BTreeMap;

use crate::snapshot::Snapshot;
use crate::vocab::{EntityType, EvidenceState, RelationType};

#[derive(Clone, Debug, Serialize)]
pub struct SnapshotStats {
    pub path: String,
    pub content_hash: String,
    pub file_bytes: u64,
    pub format_version: u32,
    pub schema_version: u32,
    pub source_event_seq: u64,
    pub nodes: u32,
    pub merged_nodes: u32,
    pub claims: u32,
    pub edges: u32,
    pub groups: u32,
    pub sources: u32,
    pub provenance_rows: u32,
    pub strings: u32,
    /// Canonical nodes by type.
    pub nodes_by_type: BTreeMap<&'static str, u64>,
    /// Canonical nodes with no claims at all, by type: coverage "unknown", never "isolated".
    pub unknown_by_type: BTreeMap<&'static str, u64>,
    pub claims_by_relation: BTreeMap<&'static str, u64>,
    pub claims_by_state: BTreeMap<&'static str, u64>,
    /// Edges of the CSR (current claims only), by relation.
    pub edges_by_relation: BTreeMap<&'static str, u64>,
    /// Concept layer: concepts, declarations with a primary concept, aggregated concept edges.
    pub concepts: u32,
    pub declarations_with_concept: u32,
    pub concept_edges: u32,
}

fn histogram(codes: &[u8], buckets: usize) -> Vec<u64> {
    codes
        .par_chunks(1 << 16)
        .map(|chunk| {
            let mut h = vec![0u64; buckets];
            for &c in chunk {
                h[c as usize % buckets] += 1;
            }
            h
        })
        .reduce(|| vec![0u64; buckets], |a, b| a.iter().zip(&b).map(|(x, y)| x + y).collect())
}

fn named<T: Copy>(all: &[T], name: fn(T) -> &'static str, counts: &[u64]) -> BTreeMap<&'static str, u64> {
    all.iter().zip(counts).filter(|(_, n)| **n > 0).map(|(v, n)| (name(*v), *n)).collect()
}

impl Snapshot {
    pub fn stats(&self) -> SnapshotStats {
        let h = *self.header();
        let n = self.num_nodes();
        let (by_type, unknown, merged) = (0..n)
            .into_par_iter()
            .fold(
                || (vec![0u64; EntityType::COUNT], vec![0u64; EntityType::COUNT], 0u32),
                |(mut t, mut u, mut m), i| {
                    if self.is_merged(i) {
                        m += 1;
                    } else {
                        let ty = self.node_type(i) as usize;
                        t[ty] += 1;
                        if self.evidence(i).is_unknown() {
                            u[ty] += 1;
                        }
                    }
                    (t, u, m)
                },
            )
            .reduce(
                || (vec![0u64; EntityType::COUNT], vec![0u64; EntityType::COUNT], 0u32),
                |a, b| {
                    let add = |x: &[u64], y: &[u64]| x.iter().zip(y).map(|(p, q)| p + q).collect::<Vec<u64>>();
                    (add(&a.0, &b.0), add(&a.1, &b.1), a.2 + b.2)
                },
            );
        let out = self.csr(crate::snapshot::Dir::Out);
        SnapshotStats {
            path: self.path().display().to_string(),
            content_hash: self.content_hash_hex(),
            file_bytes: self.file_len(),
            format_version: h.format_version,
            schema_version: h.schema_version,
            source_event_seq: h.source_event_seq,
            nodes: n,
            merged_nodes: merged,
            claims: h.num_claims,
            edges: h.num_edges,
            groups: h.num_groups,
            sources: h.num_sources,
            provenance_rows: h.num_prov,
            strings: h.num_strings,
            nodes_by_type: named(EntityType::ALL, EntityType::as_str, &by_type),
            unknown_by_type: named(EntityType::ALL, EntityType::as_str, &unknown),
            claims_by_relation: named(
                RelationType::ALL,
                RelationType::as_str,
                &histogram(self.claim_relations(), RelationType::COUNT),
            ),
            claims_by_state: named(
                EvidenceState::ALL,
                EvidenceState::as_str,
                &histogram(self.claim_states(), EvidenceState::COUNT),
            ),
            edges_by_relation: named(RelationType::ALL, RelationType::as_str, &histogram(out.rel, RelationType::COUNT)),
            concepts: self.concepts().len() as u32,
            declarations_with_concept: self.concepts().iter().map(|&c| self.concept_members(c).len() as u32).sum(),
            concept_edges: self.num_concept_edges() as u32,
        }
    }
}
