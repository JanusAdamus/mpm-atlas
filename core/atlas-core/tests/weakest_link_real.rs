//! ADR-0005 §7a on a real compiled snapshot (round 5, step 2a): no aggregated concept -> concept
//! edge is stronger than its weakest link, and every one is a candidate.  Gated: runs only with
//! `ATLAS_CHECK_SNAPSHOT=<snapshot file or dir>`, so `cargo test` stays offline and fast.
//!
//! The aggregation is recomputed independently from the CSR: for each declaration-level edge
//! u -> v (not member_of_concept) between primary members of concepts A != B, the bound is the
//! weakest of the edge and the two primary memberships (the strongest membership claim when a
//! declaration has several to the same concept, so the bound is never too strict).  Per
//! (A, B, relation) the aggregated states, sorted strongest first, must each be <= the bounds
//! sorted the same way, with the same total.

use atlas_core::{Dir, EvidenceState, NodeIx, RelationType, Snapshot};
use std::collections::BTreeMap;

fn rank(s: EvidenceState) -> u8 {
    match s {
        EvidenceState::FormallyLinked => 4,
        EvidenceState::EditoriallyReviewed => 3,
        EvidenceState::Extracted => 2,
        EvidenceState::ModelProposed => 1,
        _ => 0,
    }
}

fn membership_rank(snap: &Snapshot, decl: NodeIx, concept: NodeIx) -> u8 {
    snap.csr(Dir::Out)
        .edges_rel(decl, RelationType::MemberOfConcept)
        .iter()
        .filter(|e| e.nbr == concept && e.state.is_current())
        .map(|e| rank(e.state))
        .max()
        .expect("a primary concept has a current membership")
}

#[test]
fn aggregated_edges_are_never_stronger_than_their_weakest_link() {
    let Ok(path) = std::env::var("ATLAS_CHECK_SNAPSHOT") else {
        eprintln!("skipped: set ATLAS_CHECK_SNAPSHOT to a compiled snapshot");
        return;
    };
    let snap = Snapshot::open(&path).expect("open snapshot");
    let mut bounds: BTreeMap<(NodeIx, NodeIx, RelationType), Vec<u8>> = BTreeMap::new();
    let out = snap.csr(Dir::Out);
    for u in 0..snap.num_nodes() {
        let Some(a) = snap.primary_concept(u) else { continue };
        let mu = membership_rank(&snap, u, a);
        for e in out.edges(u).iter() {
            if e.relation == RelationType::MemberOfConcept || !e.state.is_current() {
                continue;
            }
            let Some(b) = snap.primary_concept(e.nbr) else { continue };
            if a == b {
                continue;
            }
            let bound = rank(e.state).min(mu).min(membership_rank(&snap, e.nbr, b));
            bounds.entry((a, b, e.relation)).or_default().push(bound);
        }
    }
    let (mut edges, mut stats, mut by_state) = (0usize, 0usize, BTreeMap::<EvidenceState, u64>::new());
    let mut seen = 0usize;
    for &a in snap.concepts() {
        for ce in snap.concept_edges(a) {
            edges += 1;
            assert!(ce.is_candidate(), "every aggregated edge is a candidate");
            let mut got: BTreeMap<RelationType, Vec<u8>> = BTreeMap::new();
            for s in ce.stats() {
                stats += 1;
                *by_state.entry(s.state).or_default() += u64::from(s.count);
                got.entry(s.relation).or_default().extend(std::iter::repeat_n(rank(s.state), s.count as usize));
            }
            for (rel, mut g) in got {
                let mut b = bounds.get(&(a, ce.target, rel)).cloned().unwrap_or_default();
                seen += 1;
                g.sort_unstable_by(|x, y| y.cmp(x));
                b.sort_unstable_by(|x, y| y.cmp(x));
                assert_eq!(g.len(), b.len(), "{} -> {} {rel}: totals", snap.node_id(a), snap.node_id(ce.target));
                for (x, y) in g.iter().zip(&b) {
                    assert!(x <= y, "{} -> {} {rel}: aggregated state rank {x} > weakest link {y}",
                            snap.node_id(a), snap.node_id(ce.target));
                }
            }
        }
    }
    assert_eq!(seen, bounds.len(), "every recomputed (A, B, relation) is an aggregated edge");
    eprintln!("checked {edges} aggregated edges, {stats} (relation, state) counts; declaration-level edges by state: {by_state:?}");
}
