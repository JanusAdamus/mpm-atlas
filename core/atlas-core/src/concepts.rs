//! Concept layer of the snapshot (plan `docs/plans/2026-10-17-capa-conceptual.md`).
//!
//! Input: the current `member_of_concept` claims written by `atlas/concepts.py` (declaration -> concept,
//! with a confidence).  Output, all derived and rebuilt on every compile:
//!
//! * the **primary concept** of every declaration: its membership with the highest confidence, ties to
//!   the lower Wikidata Q-number (non-Wikidata ids after every item), then the smaller id; the same rule
//!   ranks memberships in `atlas/concepts.py::_ordered`;
//! * the **concepts**: every node that is the object of a current membership, ascending, with their
//!   primary members;
//! * the **aggregated edges** concept A -> concept B: one per pair with at least one current edge
//!   (any relation except `member_of_concept`) from a primary member of A to a primary member of B,
//!   A != B, with the number of such declaration-level edges per (relation, evidence state) and the
//!   smallest claim index as a traceable example.  Nothing is invented: no members, no edge.
//!   The state of each count is the **weakest link** (ADR-0005 §7a, `EvidenceState::weakest`): the
//!   minimum of the declaration-level edge, the source member's primary membership and the target
//!   member's primary membership.  Every aggregated edge is a candidate (`ConceptEdge::is_candidate`).
//!
//! A conjunctive claim with k premises in B counts k edges (one per CSR edge).  Edges between concepts
//! themselves (Wikidata `special_case_of`, ...) are not aggregated here: they are ordinary CSR edges.

use rayon::prelude::*;

use crate::format::NONE;
use crate::vocab::{EvidenceState, RelationType};

/// Everything the concept sections hold, as built in memory.
#[derive(Default)]
pub(crate) struct ConceptLayer {
    /// node -> primary concept node, or NONE.
    pub node_concept: Vec<u32>,
    /// concept nodes, ascending.
    pub concepts: Vec<u32>,
    /// concept index -> range in `members` (len C + 1).
    pub member_off: Vec<u32>,
    /// primary members of each concept, ascending.
    pub members: Vec<u32>,
    /// concept index -> range in `edge_dst` (len C + 1).
    pub edge_off: Vec<u32>,
    /// target concept node of each aggregated edge, ascending within a concept.
    pub edge_dst: Vec<u32>,
    /// aggregated edge -> range in `stat_*` (len E + 1).
    pub stat_off: Vec<u32>,
    pub stat_rel: Vec<u8>,
    pub stat_state: Vec<u8>,
    pub stat_count: Vec<u32>,
    pub stat_example: Vec<u32>,
}

/// Out-CSR slices the projection reads (current claims only).
pub(crate) struct OutCsr<'a> {
    pub off: &'a [u32],
    pub nbr: &'a [u32],
    pub claim: &'a [u32],
    pub rel: &'a [u8],
    pub state: &'a [u8],
}

/// Wikidata Q-number of `wd:Q<n>`; anything else sorts after every item (same as `concepts._q`).
pub(crate) fn q_number(id: &str) -> u64 {
    id.strip_prefix("wd:Q")
        .filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|t| t.parse().ok())
        .unwrap_or(1 << 40)
}

/// `conf[c]`: confidence of claim c (negative if it has none); `id(n)`: entity id of node n.
pub(crate) fn build<'s>(csr: &OutCsr<'_>, conf: &[f64], id: &(dyn Fn(u32) -> &'s str + Sync)) -> ConceptLayer {
    let n = csr.off.len() - 1;
    let member = RelationType::MemberOfConcept.code();
    let range = |u: usize| csr.off[u] as usize..csr.off[u + 1] as usize;
    // (primary concept, state of that membership) per node
    let primary: Vec<(u32, u8)> = (0..n)
        .into_par_iter()
        .map(|u| {
            let r = range(u);
            let rels = &csr.rel[r.clone()];
            let lo = rels.partition_point(|&x| x < member);
            let hi = lo + rels[lo..].partition_point(|&x| x == member);
            (r.start + lo..r.start + hi)
                .min_by(|&a, &b| {
                    let (ca, cb) = (conf[csr.claim[a] as usize], conf[csr.claim[b] as usize]);
                    let (ia, ib) = (id(csr.nbr[a]), id(csr.nbr[b]));
                    cb.total_cmp(&ca).then(q_number(ia).cmp(&q_number(ib))).then(ia.cmp(ib))
                })
                .map_or((NONE, 0), |e| (csr.nbr[e], csr.state[e]))
        })
        .collect();
    let node_concept: Vec<u32> = primary.iter().map(|p| p.0).collect();
    let weakest = |a: u8, b: u8| {
        let (a, b) = (EvidenceState::from_code(a).expect("state code"), EvidenceState::from_code(b).expect("state code"));
        a.weakest(b).code()
    };

    let mut concepts: Vec<u32> = (0..csr.nbr.len()).filter(|&e| csr.rel[e] == member).map(|e| csr.nbr[e]).collect();
    concepts.par_sort_unstable();
    concepts.dedup();
    let cix = |c: u32| concepts.binary_search(&c).expect("primary concept is a membership object");

    let mut member_off = vec![0u32; concepts.len() + 1];
    for &c in node_concept.iter().filter(|&&c| c != NONE) {
        member_off[cix(c) + 1] += 1;
    }
    for i in 0..concepts.len() {
        member_off[i + 1] += member_off[i];
    }
    let mut fill = member_off.clone();
    let mut members = vec![0u32; member_off[concepts.len()] as usize];
    for (u, &c) in node_concept.iter().enumerate() {
        if c != NONE {
            let k = cix(c);
            members[fill[k] as usize] = u as u32;
            fill[k] += 1;
        }
    }

    // (source concept, target concept, relation, state, claim) of every declaration-level edge across concepts
    let (nc, pr) = (&node_concept, &primary);
    let mut cross: Vec<(u32, u32, u8, u8, u32)> = (0..n)
        .into_par_iter()
        .filter(|&u| nc[u] != NONE)
        .flat_map_iter(|u| {
            let cu = nc[u];
            range(u).filter_map(move |e| {
                let v = csr.nbr[e] as usize;
                let cv = nc[v];
                (csr.rel[e] != member && cv != NONE && cv != cu).then(|| {
                    let state = weakest(weakest(csr.state[e], pr[u].1), pr[v].1);
                    (cu, cv, csr.rel[e], state, csr.claim[e])
                })
            })
        })
        .collect();
    cross.par_sort_unstable();

    let mut layer = ConceptLayer {
        member_off,
        members,
        edge_off: vec![0u32; concepts.len() + 1],
        stat_off: vec![0],
        ..Default::default()
    };
    let mut last_pair = None;
    for run in cross.chunk_by(|x, y| (x.0, x.1, x.2, x.3) == (y.0, y.1, y.2, y.3)) {
        let (a, b, rel, state, example) = run[0]; // sorted by claim last: the smallest claim
        if last_pair != Some((a, b)) {
            last_pair = Some((a, b));
            layer.edge_dst.push(b);
            layer.edge_off[cix(a) + 1] += 1;
            layer.stat_off.push(*layer.stat_off.last().expect("starts with 0"));
        }
        layer.stat_rel.push(rel);
        layer.stat_state.push(state);
        layer.stat_count.push(run.len() as u32);
        layer.stat_example.push(example);
        *layer.stat_off.last_mut().expect("pushed above") += 1;
    }
    for k in 0..concepts.len() {
        layer.edge_off[k + 1] += layer.edge_off[k];
    }
    layer.node_concept = node_concept;
    layer.concepts = concepts;
    layer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_concepts_members_and_aggregated_edges() {
        let (dep, mem) = (RelationType::DependsOnLogically.code(), RelationType::MemberOfConcept.code());
        // node 0: three edges to 1 (two extracted, one model_proposed), memberships 3 and 4 tied at 0.5
        // node 1: member of 4; node 2: edges to 1 (same concept) and 5, member of 4; node 5: member of 6
        let off = [0, 5, 6, 9, 9, 9, 10, 10];
        let nbr = [1, 1, 1, 3, 4, 4, 1, 5, 4, 6];
        let claim = [0, 8, 9, 3, 4, 5, 1, 2, 6, 7];
        let rel = [dep, dep, dep, mem, mem, mem, dep, dep, mem, mem];
        let state = [0, 0, 1, 0, 1, 1, 0, 0, 1, 0];
        let conf = [-1.0, -1.0, -1.0, 0.5, 0.5, 0.9, 0.3, 1.0, -1.0, -1.0];
        let ids = ["d0", "d1", "d2", "wd:Q5", "wd:Q10", "d5", "concept:Mathlib/X"];
        let csr = OutCsr { off: &off, nbr: &nbr, claim: &claim, rel: &rel, state: &state };
        let l = build(&csr, &conf, &|n| ids[n as usize]);
        assert_eq!(l.node_concept, vec![3, 4, 4, NONE, NONE, 6, NONE], "tie at 0.5 goes to the lower Q-number");
        assert_eq!(l.concepts, vec![3, 4, 6]);
        assert_eq!((l.member_off.as_slice(), l.members.as_slice()), (&[0, 1, 3, 4][..], &[0, 1, 2, 5][..]));
        assert_eq!((l.edge_off.as_slice(), l.edge_dst.as_slice()), (&[0, 1, 2, 2][..], &[4, 6][..]));
        // weakest link: node 1's membership is model_proposed, so all three edges 0 -> 1 count as model_proposed;
        // 2 -> 5 is an extracted edge from a model_proposed member (node 2)
        assert_eq!(l.stat_off, vec![0, 1, 2]);
        assert_eq!((l.stat_state.as_slice(), l.stat_count.as_slice()), (&[1, 1][..], &[3, 1][..]));
        assert_eq!(l.stat_example, vec![0, 2], "the smallest claim of each (relation, state)");
        assert!(l.stat_rel.iter().all(|&r| r == dep), "membership edges are never aggregated");
    }

    #[test]
    fn aggregated_edge_takes_the_weakest_link() {
        let (dep, mem) = (RelationType::DependsOnLogically.code(), RelationType::MemberOfConcept.code());
        let (ext, prop, formal) = (
            EvidenceState::Extracted.code(),
            EvidenceState::ModelProposed.code(),
            EvidenceState::FormallyLinked.code(),
        );
        // 0 -> 1 is a formally_linked dependency; 0 is an extracted member of 2, 1 a model_proposed member of 3
        let off = [0, 2, 3, 3, 3];
        let (nbr, claim) = ([1, 2, 3], [0, 1, 2]);
        let (rel, state) = ([dep, mem, mem], [formal, ext, prop]);
        let conf = [-1.0, 0.9, 0.9];
        let ids = ["d0", "d1", "wd:Q1", "wd:Q2"];
        let csr = OutCsr { off: &off, nbr: &nbr, claim: &claim, rel: &rel, state: &state };
        let l = build(&csr, &conf, &|n| ids[n as usize]);
        assert_eq!(l.edge_dst, vec![3]);
        assert_eq!(l.stat_state, vec![prop], "formally_linked dependency + model_proposed membership -> model_proposed");
        let mut state2 = state;
        (state2[0], state2[2]) = (ext, ext);
        let csr = OutCsr { state: &state2, ..csr };
        assert_eq!(build(&csr, &conf, &|n| ids[n as usize]).stat_state, vec![ext]);
    }

    #[test]
    fn q_numbers_match_python() {
        assert_eq!(q_number("wd:Q42"), 42);
        assert_eq!(q_number("wd:Qx1"), 1 << 40);
        assert_eq!(q_number("concept:Mathlib/Q1"), 1 << 40);
    }
}
