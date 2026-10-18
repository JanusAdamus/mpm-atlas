//! Per-snapshot precomputation for the search: connected components (instant
//! abstention when two nodes are not connected at all) and ALT landmarks
//! (Goldberg & Harrelson 2005): hop distances from a few nodes in an undirected
//! graph of claims.  `|d(L,v) - d(L,t)|` is a lower bound on the number of steps of
//! *any* route v -> t that walks a subset of those edges.
//!
//! Two landmark sets:
//! - `all`: every edge.  Valid for any route (also `min_strength = 0`).
//! - `semantic`: only relations with a step of strength >= 1 in some direction.  A
//!   route authorized at `min_strength >= 1` never uses another relation (strength
//!   never increases along a route), so the bound stays admissible, and it is much
//!   tighter: area, publication and citation hubs no longer shortcut every pair.

use atlas_core::{Dir, EdgeSlice, RelationType, Snapshot};
use rayon::prelude::*;

use crate::algebra::{CompositionTable, NONE, Way};

/// Hop distance cap stored in a byte; also "unreachable".
pub const FAR: u8 = u8::MAX;

/// Which relation codes an index walks.
type Keep = [bool; 32];

/// Hop distances from a few landmarks, node-major: `dist[v * L + i]`.
pub struct Landmarks {
    pub nodes: Vec<u32>,
    dist: Vec<u8>,
}

impl Landmarks {
    fn build(snap: &Snapshot, num: usize, keep: &Keep, component: &[u32]) -> Landmarks {
        let nodes = pick_landmarks(snap, component, num, keep);
        let n = snap.num_nodes() as usize;
        let rows: Vec<Vec<u8>> = nodes.par_iter().map(|&l| bfs(snap, l, keep)).collect();
        let l = nodes.len();
        let mut dist = vec![FAR; n * l];
        if l > 0 {
            dist.par_chunks_mut(l).enumerate().for_each(|(v, row)| {
                for (i, r) in rows.iter().enumerate() {
                    row[i] = r[v];
                }
            });
        }
        Landmarks { nodes, dist }
    }

    #[inline]
    fn row(&self, x: u32) -> &[u8] {
        let l = self.nodes.len();
        &self.dist[x as usize * l..][..l]
    }

    /// `(lb(v, a), lb(v, b))` in one pass over v's row.
    #[inline]
    fn lb2(&self, v: u32, a: u32, b: u32) -> (u32, u32) {
        if self.nodes.is_empty() {
            return (0, 0);
        }
        let (rv, ra, rb) = (self.row(v), self.row(a), self.row(b));
        let (mut da, mut db) = (0u8, 0u8);
        for i in 0..rv.len() {
            let x = rv[i];
            if x == FAR {
                continue;
            }
            if ra[i] != FAR {
                da = da.max(x.abs_diff(ra[i]));
            }
            if rb[i] != FAR {
                db = db.max(x.abs_diff(rb[i]));
            }
        }
        (da as u32, db as u32)
    }

    fn memory_bytes(&self) -> usize {
        self.dist.len() + self.nodes.len() * 4
    }
}

pub struct GraphIndex {
    /// Component id per node over every edge (smallest node index of the component).
    pub component: Vec<u32>,
    pub all: Landmarks,
    /// Landmarks over the relations that can carry strength >= 1 (None: not built).
    pub semantic: Option<Landmarks>,
}

impl GraphIndex {
    /// Landmarks over every edge only.
    pub fn build(snap: &Snapshot, num_landmarks: usize) -> GraphIndex {
        let keep = [true; 32];
        let component = components(snap, &keep);
        let all = Landmarks::build(snap, num_landmarks, &keep, &component);
        GraphIndex { component, all, semantic: None }
    }

    /// Both landmark sets; the semantic one follows `table`.
    pub fn build_typed(snap: &Snapshot, num_landmarks: usize, table: &CompositionTable) -> GraphIndex {
        let keep = semantic_relations(table);
        let (base, semantic) = rayon::join(
            || GraphIndex::build(snap, num_landmarks),
            || Landmarks::build(snap, num_landmarks, &keep, &components(snap, &keep)),
        );
        GraphIndex { semantic: Some(semantic), ..base }
    }

    pub fn connected(&self, a: u32, b: u32) -> bool {
        self.component[a as usize] == self.component[b as usize]
    }

    /// Lower bound on the hops between `a` and `b` over every edge (0 without landmarks).
    #[inline]
    pub fn hop_lb(&self, a: u32, b: u32) -> u32 {
        self.all.lb2(a, b, b).0
    }

    /// `(hop_lb(v, a), hop_lb(v, b))`; `semantic` selects the bound valid only for
    /// routes authorized at `min_strength >= 1` (`all` if it was not built).
    #[inline]
    pub fn hop_lb2(&self, v: u32, a: u32, b: u32, semantic: bool) -> (u32, u32) {
        match (&self.semantic, semantic) {
            (Some(s), true) => s.lb2(v, a, b),
            _ => self.all.lb2(v, a, b),
        }
    }

    pub fn memory_bytes(&self) -> usize {
        self.component.len() * 4 + self.all.memory_bytes() + self.semantic.as_ref().map_or(0, Landmarks::memory_bytes)
    }
}

/// Relation codes with a step of strength >= 1 in either direction.
fn semantic_relations(table: &CompositionTable) -> Keep {
    let mut keep = [false; 32];
    for rel in RelationType::ALL {
        keep[rel.code() as usize] = [Way::Forward, Way::Converse].iter().any(|&w| {
            let k = table.step_kind(*rel, w);
            k != NONE && table.strength(k) >= 1
        });
    }
    keep
}

/// Union-find over the out-CSR (each undirected edge appears there once).
fn components(snap: &Snapshot, keep: &Keep) -> Vec<u32> {
    let n = snap.num_nodes() as usize;
    let mut parent: Vec<u32> = (0..n as u32).collect();
    fn find(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            let g = p[p[x as usize] as usize];
            p[x as usize] = g;
            x = g;
        }
        x
    }
    let out = snap.csr(Dir::Out);
    for u in 0..n as u32 {
        let e = out.edges(u);
        for (i, &v) in e.nbr.iter().enumerate() {
            if !keep[e.rel[i] as usize] {
                continue;
            }
            let (a, b) = (find(&mut parent, u), find(&mut parent, v));
            if a != b {
                // smaller index wins: the id is then deterministic
                let (lo, hi) = if a < b { (a, b) } else { (b, a) };
                parent[hi as usize] = lo;
            }
        }
    }
    (0..n as u32).map(|x| find(&mut parent, x)).collect()
}

/// Deterministic spread: the highest-degree node of the largest component first,
/// then nodes chosen by a fixed hash among that component's non-isolated nodes.
/// ponytail: no farthest-point selection (sequential BFS per landmark); add it if
/// the measured pruning is worth the build time.
fn pick_landmarks(snap: &Snapshot, component: &[u32], k: usize, keep: &Keep) -> Vec<u32> {
    let n = component.len();
    if k == 0 || n == 0 {
        return Vec::new();
    }
    let (out, inn) = (snap.csr(Dir::Out), snap.csr(Dir::In));
    let kept = |e: EdgeSlice| e.rel.iter().filter(|&&r| keep[r as usize]).count();
    let degree = |v: u32| kept(out.edges(v)) + kept(inn.edges(v));
    let mut size = std::collections::HashMap::new();
    for &c in component {
        *size.entry(c).or_insert(0usize) += 1;
    }
    let (&giant, _) = size.iter().max_by_key(|(c, s)| (**s, std::cmp::Reverse(**c))).expect("n > 0");
    let members: Vec<u32> = (0..n as u32).filter(|&v| component[v as usize] == giant && degree(v) > 0).collect();
    if members.is_empty() {
        return Vec::new();
    }
    let hub = *members.iter().max_by_key(|&&v| (degree(v), std::cmp::Reverse(v))).expect("non-empty");
    let mut picked = vec![hub];
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    while picked.len() < k.min(members.len()) {
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9).wrapping_add(0x94D0_49BB_1331_11EB);
        let v = members[(h >> 17) as usize % members.len()];
        if !picked.contains(&v) {
            picked.push(v);
        }
    }
    picked
}

/// Undirected BFS over both CSR directions (kept relations), distances capped below FAR.
fn bfs(snap: &Snapshot, src: u32, keep: &Keep) -> Vec<u8> {
    let n = snap.num_nodes() as usize;
    let (out, inn) = (snap.csr(Dir::Out), snap.csr(Dir::In));
    let mut dist = vec![FAR; n];
    dist[src as usize] = 0;
    let mut frontier = vec![src];
    let mut next = Vec::new();
    let mut d = 0u8;
    while !frontier.is_empty() && d < FAR - 1 {
        d += 1;
        for &u in &frontier {
            for e in [out.edges(u), inn.edges(u)] {
                for (i, &v) in e.nbr.iter().enumerate() {
                    if keep[e.rel[i] as usize] && dist[v as usize] == FAR {
                        dist[v as usize] = d;
                        next.push(v);
                    }
                }
            }
        }
        std::mem::swap(&mut frontier, &mut next);
        next.clear();
    }
    dist
}
