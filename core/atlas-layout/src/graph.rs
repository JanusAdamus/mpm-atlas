//! Undirected weighted graph in CSR form (both arcs of every edge stored), the
//! input of Leiden and of the force layout.  Aggregation by a partition gives the
//! coarse graphs of both.
//!
//! From a snapshot: one undirected edge per pair of distinct current-claim
//! endpoints whose relation belongs to the chosen layers.  A conjunction with k
//! premises contributes k edges of weight 1/k each, so one claim weighs 1 however
//! many premises it has.  Parallel edges add up.  Self loops are dropped.

use atlas_core::{Dir, Snapshot};
use rayon::prelude::*;

use crate::util::{CHUNK, group_by};

#[derive(Clone, Debug, Default)]
pub struct Graph {
    /// `nbr[offsets[v]..offsets[v+1]]` are the neighbours of `v`, sorted, distinct, never `v`.
    pub offsets: Vec<usize>,
    pub nbr: Vec<u32>,
    pub w: Vec<f64>,
    /// Weight inside an aggregated node (each undirected edge counted once).
    pub self_w: Vec<f64>,
    /// Original nodes represented: 1 at the base level (the CPM size).
    pub node_w: Vec<f64>,
    /// Layout mass, ForceAtlas2 style: Σ (degree + 1) over the original nodes
    /// represented; 0 for a node with no edges.
    pub mass: Vec<f64>,
}

impl Graph {
    pub fn n(&self) -> usize {
        self.node_w.len()
    }

    pub fn num_edges(&self) -> usize {
        self.nbr.len() / 2
    }

    #[inline]
    pub fn neighbors(&self, v: u32) -> (&[u32], &[f64]) {
        let r = self.offsets[v as usize]..self.offsets[v as usize + 1];
        (&self.nbr[r.clone()], &self.w[r])
    }

    pub fn degree(&self, v: u32) -> usize {
        self.offsets[v as usize + 1] - self.offsets[v as usize]
    }

    /// Weighted degree `k_v` (self loops count twice, the modularity convention).
    pub fn strength(&self) -> Vec<f64> {
        (0..self.n())
            .into_par_iter()
            .map(|v| self.neighbors(v as u32).1.iter().sum::<f64>() + 2.0 * self.self_w[v])
            .collect()
    }

    /// From an explicit edge list (tests, benchmarks).  Duplicates add up; self loops are dropped.
    pub fn from_edges(n: usize, edges: &[(u32, u32, f64)]) -> Graph {
        let mut adj: Vec<Vec<(u32, f64)>> = vec![Vec::new(); n];
        for &(a, b, w) in edges {
            assert!(
                (a as usize) < n && (b as usize) < n,
                "edge endpoint out of range"
            );
            assert!(w > 0.0 && w.is_finite(), "edge weights must be positive");
            if a != b {
                adj[a as usize].push((b, w));
                adj[b as usize].push((a, w));
            }
        }
        let (offsets, nbr, w, self_w) =
            build_csr(n, |v, buf| buf.extend_from_slice(&adj[v as usize]));
        Self::with_base_weights(offsets, nbr, w, self_w, None)
    }

    /// Current-claim edges of `snap` whose relation bit is in `relation_mask`
    /// (`1 << RelationType::code()`).  Nodes with `active[v] == false` (merged
    /// redirects, excluded nodes) get no edges and mass 0.
    pub fn from_snapshot(snap: &Snapshot, relation_mask: u32, active: &[bool]) -> Graph {
        let n = snap.num_nodes() as usize;
        assert_eq!(active.len(), n);
        let (out, inn) = (snap.csr(Dir::Out), snap.csr(Dir::In));
        let (offsets, nbr, w, self_w) = build_csr(n, |v, buf| {
            if !active[v as usize] {
                return;
            }
            for csr in [&out, &inn] {
                let r = csr.range(v);
                for i in r {
                    let (u, rel) = (csr.nbr[i], csr.rel[i]);
                    if relation_mask & (1 << rel) == 0 || u == v || !active[u as usize] {
                        continue;
                    }
                    buf.push((u, 1.0 / snap.conjunct_count(csr.claim[i]).max(1) as f64));
                }
            }
        });
        Self::with_base_weights(offsets, nbr, w, self_w, Some(active))
    }

    fn with_base_weights(
        offsets: Vec<usize>,
        nbr: Vec<u32>,
        w: Vec<f64>,
        self_w: Vec<f64>,
        active: Option<&[bool]>,
    ) -> Graph {
        let n = offsets.len() - 1;
        let mass = (0..n)
            .map(|v| match active {
                Some(a) if !a[v] => 0.0,
                // No edges: nothing places the node, so it stays out of the force layout.
                _ if offsets[v + 1] == offsets[v] => 0.0,
                _ => (offsets[v + 1] - offsets[v]) as f64 + 1.0,
            })
            .collect();
        Graph {
            offsets,
            nbr,
            w,
            self_w,
            node_w: vec![1.0; n],
            mass,
        }
    }

    /// The graph of communities: node `c` stands for the members of `c`; weights,
    /// sizes and masses add up; edges inside `c` become its self weight.
    pub fn aggregate(&self, membership: &[u32], count: usize) -> Graph {
        assert_eq!(membership.len(), self.n());
        let (order, start) = group_by(membership, count);
        let (offsets, nbr, w, self_arcs) = build_csr(count, |c, buf| {
            for &v in &order[start[c as usize]..start[c as usize + 1]] {
                let (ns, ws) = self.neighbors(v);
                buf.extend(
                    ns.iter()
                        .zip(ws)
                        .map(|(&u, &x)| (membership[u as usize], x)),
                );
            }
        });
        let sum = |f: &(dyn Fn(u32) -> f64 + Sync)| -> Vec<f64> {
            (0..count)
                .into_par_iter()
                .map(|c| order[start[c]..start[c + 1]].iter().map(|&v| f(v)).sum())
                .collect()
        };
        let inner = sum(&|v| self.self_w[v as usize]);
        // Every internal edge was seen from both ends.
        let self_w = inner
            .iter()
            .zip(&self_arcs)
            .map(|(a, b)| a + b / 2.0)
            .collect();
        Graph {
            offsets,
            nbr,
            w,
            self_w,
            node_w: sum(&|v| self.node_w[v as usize]),
            mass: sum(&|v| self.mass[v as usize]),
        }
    }
}

type Csr = (Vec<usize>, Vec<u32>, Vec<f64>, Vec<f64>);
/// (degree, neighbours, weights, self weight) of one chunk of nodes.
type Chunk = (Vec<u32>, Vec<u32>, Vec<f64>, Vec<f64>);

/// Build a CSR from per-node arc lists (any order, duplicates allowed).  Arcs to
/// the node itself are summed into the fourth vector instead.  Deterministic:
/// fixed chunks, stable sorts, in-order concatenation.
pub(crate) fn build_csr(n: usize, fill: impl Fn(u32, &mut Vec<(u32, f64)>) + Sync) -> Csr {
    let chunks: Vec<Chunk> = (0..n.div_ceil(CHUNK))
        .into_par_iter()
        .map(|ci| {
            let (lo, hi) = (ci * CHUNK, ((ci + 1) * CHUNK).min(n));
            let mut buf: Vec<(u32, f64)> = Vec::new();
            let (mut deg, mut nbr, mut w, mut selfs) = (
                Vec::with_capacity(hi - lo),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
            for v in lo..hi {
                buf.clear();
                fill(v as u32, &mut buf);
                buf.sort_by_key(|a| a.0);
                let before = nbr.len();
                let mut s = 0.0;
                for &(u, x) in &buf {
                    if u as usize == v {
                        s += x;
                    } else if nbr.len() > before && *nbr.last().unwrap() == u {
                        *w.last_mut().unwrap() += x;
                    } else {
                        nbr.push(u);
                        w.push(x);
                    }
                }
                deg.push((nbr.len() - before) as u32);
                selfs.push(s);
            }
            (deg, nbr, w, selfs)
        })
        .collect();
    let total: usize = chunks.iter().map(|c| c.1.len()).sum();
    let mut offsets = Vec::with_capacity(n + 1);
    offsets.push(0usize);
    let (mut nbr, mut w, mut self_w) = (
        Vec::with_capacity(total),
        Vec::with_capacity(total),
        Vec::with_capacity(n),
    );
    for (deg, cn, cw, cs) in chunks {
        for d in deg {
            offsets.push(offsets.last().unwrap() + d as usize);
        }
        nbr.extend_from_slice(&cn);
        w.extend_from_slice(&cw);
        self_w.extend_from_slice(&cs);
    }
    (offsets, nbr, w, self_w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_preserves_weight_and_mass() {
        // two triangles joined by one edge
        let e = [
            (0, 1, 1.0),
            (1, 2, 1.0),
            (0, 2, 1.0),
            (3, 4, 1.0),
            (4, 5, 1.0),
            (3, 5, 1.0),
            (2, 3, 2.0),
            (0, 1, 1.0),
        ];
        let g = Graph::from_edges(6, &e);
        assert_eq!(g.num_edges(), 7, "duplicate (0,1) merged");
        assert_eq!(g.neighbors(0), (&[1, 2][..], &[2.0, 1.0][..]));
        let total: f64 = g.strength().iter().sum();
        let a = g.aggregate(&[0, 0, 0, 1, 1, 1], 2);
        assert_eq!(a.neighbors(0), (&[1][..], &[2.0][..]));
        assert_eq!(a.self_w, vec![4.0, 3.0]);
        assert_eq!(a.node_w, vec![3.0, 3.0]);
        assert_eq!(a.strength().iter().sum::<f64>(), total);
        assert_eq!(a.mass.iter().sum::<f64>(), g.mass.iter().sum::<f64>());
    }
}
