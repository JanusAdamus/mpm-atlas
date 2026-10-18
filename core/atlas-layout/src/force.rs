//! ForceAtlas2 (Jacomy et al. 2014) with Barnes-Hut repulsion, in parallel.
//!
//! Forces on node i (mass m_i = degree + 1, summed for aggregates):
//!   repulsion   k_r m_i m_j / d       from every other node (Barnes-Hut, opening angle θ)
//!   attraction  w_ij d                towards each neighbour (linear mode)
//!   gravity     k_g m_i d             towards the origin ("strong gravity"; k_g m_i
//!               without `strong_gravity`).  Constant gravity lets repulsion from
//!               the giant component push small components far out, which shrank
//!               the real map to 4% of the unit square (README, "Layout")
//!   anchor      k_a m_i |p_i − a_i|   towards a previous position (warm start only)
//! Speeds follow Gephi's adaptive scheme (global speed from swinging / traction,
//! per-node damping by its own swinging).
//!
//! The quadtree is built over Morton-sorted points; forces are computed per node
//! in parallel, each node summing its own terms in a fixed order, so a run is
//! bit-identical for any thread count.

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::graph::Graph;
use crate::util::{CHUNK, det_sum2, mix};

#[derive(Clone, Debug)]
pub struct ForceParams {
    pub theta: f64,
    /// k_r
    pub scaling: f64,
    /// k_g
    pub gravity: f64,
    /// Gravity grows with distance (k_g m_i d) instead of staying constant (k_g m_i).
    pub strong_gravity: bool,
    pub jitter_tolerance: f64,
    /// k_a (warm start).
    pub anchor: f64,
}

impl Default for ForceParams {
    fn default() -> Self {
        // Gephi's defaults for large graphs.
        ForceParams {
            theta: 1.2,
            scaling: 2.0,
            gravity: 1.0,
            strong_gravity: true,
            jitter_tolerance: 1.0,
            anchor: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct RunStats {
    pub nodes: usize,
    pub iterations: u32,
    pub secs: f64,
    /// Mean |displacement| per node in the last iteration (layout units).
    pub last_step: f64,
    /// Time in quadtree builds, repulsion, and the rest (attraction, gravity, moves).
    pub secs_tree: f64,
    pub secs_repulsion: f64,
    /// Largest leaf of the last quadtree (points compared pairwise).
    pub max_leaf: usize,
}

const LEAF: usize = 8;
/// 24 bits per axis: cells down to 2^-24 of the bounding box, so far outliers do
/// not squeeze the core into a few overfull leaves.
const MAX_DEPTH: u32 = 24;
/// The sequential top of the tree splits cells until they hold at most
/// max(n / PAR_CELLS, PAR_MIN_CELL) points; those subtrees are built in parallel.
/// (A fixed depth leaves few, huge cells when outliers stretch the bounding box.)
const PAR_CELLS: usize = 512;
const PAR_MIN_CELL: usize = 2048;

#[derive(Clone, Copy)]
struct QNode {
    cx: f64,
    cy: f64,
    mass: f64,
    x0: f64,
    y0: f64,
    size: f64,
    /// Children: `first..first+count` in `nodes`; leaf when `count == 0`.
    first: u32,
    count: u8,
    lo: u32,
    hi: u32,
}

impl QNode {
    fn new(x0: f64, y0: f64, size: f64) -> QNode {
        QNode {
            cx: 0.0,
            cy: 0.0,
            mass: 0.0,
            x0,
            y0,
            size,
            first: 0,
            count: 0,
            lo: 0,
            hi: 0,
        }
    }
}

/// Barnes-Hut quadtree over Morton-sorted points.  Rebuilt every iteration into
/// the same buffers: fresh multi-megabyte vectors per iteration cost more in page
/// faults than the build itself on Windows (README, "Layout").
#[derive(Default)]
struct QuadTree {
    nodes: Vec<QNode>,
    /// (Morton code, original node) of each point, sorted.
    keys: Vec<(u64, u32)>,
    /// (x, y, mass) of each sorted point.
    pts: Vec<[f64; 3]>,
}

/// Morton-sorted points, shared read-only by the subtree builders.
struct Sorted<'a> {
    keys: &'a [(u64, u32)],
    pts: &'a [[f64; 3]],
}

/// Work left by the sequential top of the tree.
#[derive(Default)]
struct Top {
    /// (node, lo, hi, depth) of cells to build in parallel.
    frontier: Vec<(usize, usize, usize, u32)>,
    /// Cells with at most this many points go to the frontier.
    cell: usize,
    /// Internal top nodes, parents before children; summed bottom-up at the end.
    internal: Vec<usize>,
}

impl Sorted<'_> {
    /// Fill `nodes[at]`, the cell `lo..hi` at `depth`: a leaf, or children appended
    /// to `nodes` (indices local to `nodes`).  With `top`, cells small enough are
    /// left for parallel tasks once small enough, and internal nodes are summed later.
    fn fill(
        &self,
        nodes: &mut Vec<QNode>,
        at: usize,
        lo: usize,
        hi: usize,
        depth: u32,
        top: Option<&mut Top>,
    ) {
        if hi - lo <= LEAF || depth == MAX_DEPTH {
            let (mut m, mut sx, mut sy) = (0.0, 0.0, 0.0);
            for i in lo..hi {
                m += self.pts[i][2];
                sx += self.pts[i][2] * self.pts[i][0];
                sy += self.pts[i][2] * self.pts[i][1];
            }
            let n = &mut nodes[at];
            n.mass = m;
            (n.cx, n.cy) = if m > 0.0 {
                (sx / m, sy / m)
            } else {
                (n.x0 + n.size / 2.0, n.y0 + n.size / 2.0)
            };
            (n.lo, n.hi) = (lo as u32, hi as u32);
            return;
        }
        let mut top = top;
        if let Some(t) = top.as_deref_mut() {
            if hi - lo <= t.cell {
                t.frontier.push((at, lo, hi, depth));
                return;
            }
            t.internal.push(at);
        }
        let shift = 2 * (MAX_DEPTH - 1 - depth);
        let slice = &self.keys[lo..hi];
        let mut b = [lo; 5];
        for q in 1..4u64 {
            b[q as usize] = lo + slice.partition_point(|k| (k.0 >> shift) & 3 < q);
        }
        b[4] = hi;
        let (x0, y0, half) = (nodes[at].x0, nodes[at].y0, nodes[at].size / 2.0);
        let first = nodes.len();
        let mut kids = [(0usize, 0usize, 0usize); 4];
        let mut nk = 0;
        for q in 0..4usize {
            if b[q] < b[q + 1] {
                // Morton bit order: x is the low bit of each pair.
                let (ox, oy) = ((q & 1) as f64 * half, (q >> 1) as f64 * half);
                kids[nk] = (nodes.len(), b[q], b[q + 1]);
                nk += 1;
                nodes.push(QNode::new(x0 + ox, y0 + oy, half));
            }
        }
        nodes[at].first = first as u32;
        nodes[at].count = nk as u8;
        for &(k, a, z) in &kids[..nk] {
            self.fill(nodes, k, a, z, depth + 1, top.as_deref_mut());
        }
        if top.is_none() {
            sum_children(nodes, at);
        }
    }
}

/// Mass and centre of mass of an internal node from its children.
fn sum_children(nodes: &mut [QNode], at: usize) {
    let n = nodes[at];
    let (mut m, mut sx, mut sy) = (0.0, 0.0, 0.0);
    for c in &nodes[n.first as usize..n.first as usize + n.count as usize] {
        m += c.mass;
        sx += c.mass * c.cx;
        sy += c.mass * c.cy;
    }
    let half = n.size / 2.0;
    let t = &mut nodes[at];
    t.mass = m;
    (t.cx, t.cy) = if m > 0.0 {
        (sx / m, sy / m)
    } else {
        (n.x0 + half, n.y0 + half)
    };
}

impl QuadTree {
    #[cfg(test)]
    fn build(pos: &[[f64; 2]], mass: &[f64], members: &[u32]) -> QuadTree {
        let mut t = QuadTree::default();
        t.rebuild(pos, mass, members);
        t
    }

    /// Morton sort in parallel, the top of the tree sequentially, then each
    /// frontier subtree in parallel, concatenated in frontier order: the same tree
    /// for any thread count.
    fn rebuild(&mut self, pos: &[[f64; 2]], mass: &[f64], members: &[u32]) {
        let t_all = std::time::Instant::now();
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &v in members {
            let p = pos[v as usize];
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let size = (x1 - x0).max(y1 - y0).max(1e-9) * (1.0 + 1e-9);
        let scale = ((1u64 << MAX_DEPTH) as f64 - 1.0) / size;
        let key = |v: u32| {
            let p = pos[v as usize];
            let qx = ((p[0] - x0) * scale) as u32;
            let qy = ((p[1] - y0) * scale) as u32;
            (morton(qx, qy), v)
        };
        if self.keys.len() == members.len() {
            // Same points as last iteration: recompute codes in the previous Morton
            // order, which is nearly sorted already (the sort result does not
            // depend on input order: (code, node) keys are distinct).
            self.keys.par_iter_mut().for_each(|k| *k = key(k.1));
        } else {
            members
                .par_iter()
                .map(|&v| key(v))
                .collect_into_vec(&mut self.keys);
        }
        let t_keys = t_all.elapsed().as_secs_f64();
        let t0 = std::time::Instant::now();
        self.keys.par_sort_unstable();
        let t_sort = t0.elapsed().as_secs_f64();
        let keys = &self.keys;
        keys.par_iter()
            .map(|k| {
                let p = pos[k.1 as usize];
                [p[0], p[1], mass[k.1 as usize]]
            })
            .collect_into_vec(&mut self.pts);
        let t_cols = t0.elapsed().as_secs_f64() - t_sort;
        let mut nodes = std::mem::take(&mut self.nodes);
        nodes.clear();
        let s = Sorted {
            keys,
            pts: &self.pts,
        };
        nodes.push(QNode::new(x0, y0, size));
        let mut top = Top {
            cell: (members.len() / PAR_CELLS).max(PAR_MIN_CELL),
            ..Default::default()
        };
        let t0 = std::time::Instant::now();
        s.fill(&mut nodes, 0, 0, members.len(), 0, Some(&mut top));
        let t_top = t0.elapsed().as_secs_f64();
        let t0 = std::time::Instant::now();
        let subs: Vec<Vec<QNode>> = top
            .frontier
            .par_iter()
            .map(|&(at, lo, hi, depth)| {
                let mut sub = vec![nodes[at]];
                s.fill(&mut sub, 0, lo, hi, depth, None);
                sub
            })
            .collect();
        // sub[0] replaces nodes[at]; sub[k] (k >= 1) goes to base + k - 1.  Bases are
        // a prefix sum, so the copies run in parallel into disjoint slices.
        let fix = |mut n: QNode, base: usize| {
            if n.count > 0 {
                n.first = (base + n.first as usize - 1) as u32;
            }
            n
        };
        let top_len = nodes.len();
        let mut bases = Vec::with_capacity(subs.len());
        let mut total = top_len;
        for sub in &subs {
            bases.push(total);
            total += sub.len() - 1;
        }
        for ((&(at, ..), sub), &base) in top.frontier.iter().zip(&subs).zip(&bases) {
            nodes[at] = fix(sub[0], base);
        }
        nodes.resize(total, QNode::new(0.0, 0.0, 0.0));
        let mut rest = &mut nodes[top_len..];
        let mut jobs = Vec::with_capacity(subs.len());
        for (sub, &base) in subs.iter().zip(&bases) {
            let (head, tail) = std::mem::take(&mut rest).split_at_mut(sub.len() - 1);
            jobs.push((head, sub, base));
            rest = tail;
        }
        jobs.into_par_iter().for_each(|(dst, sub, base)| {
            for (d, &n) in dst.iter_mut().zip(&sub[1..]) {
                *d = fix(n, base);
            }
        });
        for &i in top.internal.iter().rev() {
            sum_children(&mut nodes, i);
        }
        if std::env::var_os("ATLAS_FORCE_TRACE").is_some() {
            eprintln!(
                "tree n={} all {:.4} keys {t_keys:.4} sort {t_sort:.4} cols {t_cols:.4} top {t_top:.4} subs+merge {:.4} frontier {} nodes {}",
                members.len(),
                t_all.elapsed().as_secs_f64(),
                t0.elapsed().as_secs_f64(),
                top.frontier.len(),
                nodes.len()
            );
        }
        self.nodes = nodes;
    }

    /// Repulsion on sorted point `k` (k_r factored out, multiplied by m_k).
    fn repulsion(&self, k: usize, theta2: f64, stack: &mut Vec<u32>) -> [f64; 2] {
        let (x, y) = (self.pts[k][0], self.pts[k][1]);
        let (mut fx, mut fy) = (0.0, 0.0);
        stack.clear();
        stack.push(0);
        while let Some(at) = stack.pop() {
            let n = &self.nodes[at as usize];
            if n.count == 0 {
                for j in n.lo as usize..n.hi as usize {
                    if j == k {
                        continue;
                    }
                    let (dx, dy) = (x - self.pts[j][0], y - self.pts[j][1]);
                    let d2 = dx * dx + dy * dy;
                    if d2 > 1e-18 {
                        let f = self.pts[j][2] / d2;
                        fx += dx * f;
                        fy += dy * f;
                    } else {
                        // Coincident points: a tiny deterministic push apart.
                        let h = mix((k as u64) << 32 | j as u64);
                        fx += ((h & 0xFFFF) as f64 / 65535.0 - 0.5) * self.pts[j][2];
                        fy += (((h >> 16) & 0xFFFF) as f64 / 65535.0 - 0.5) * self.pts[j][2];
                    }
                }
                continue;
            }
            let (dx, dy) = (x - n.cx, y - n.cy);
            let d2 = dx * dx + dy * dy;
            let inside = x >= n.x0 && x < n.x0 + n.size && y >= n.y0 && y < n.y0 + n.size;
            if !inside && n.size * n.size < theta2 * d2 {
                let f = n.mass / d2;
                fx += dx * f;
                fy += dy * f;
            } else {
                stack.extend(n.first..n.first + n.count as u32);
            }
        }
        [fx * self.pts[k][2], fy * self.pts[k][2]]
    }
}

fn spread(v: u32) -> u64 {
    let mut x = v as u64;
    x = (x | (x << 16)) & 0x0000_FFFF_0000_FFFF;
    x = (x | (x << 8)) & 0x00FF_00FF_00FF_00FF;
    x = (x | (x << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    x = (x | (x << 2)) & 0x3333_3333_3333_3333;
    (x | (x << 1)) & 0x5555_5555_5555_5555
}

fn morton(x: u32, y: u32) -> u64 {
    spread(x) | (spread(y) << 1)
}

/// Anchors for a warm start: target position per node, used where `has[v]`.
pub struct Anchors<'a> {
    pub pos: &'a [[f64; 2]],
    pub has: &'a [bool],
}

/// Graphs with fewer active nodes run on one thread: a parallel step costs more
/// in wake-ups than it saves (measured: 1.4 ms per quadtree of 719 nodes on 16
/// threads under load, README "Layout").  Results are the same either way.
const SEQUENTIAL_BELOW: usize = 16_384;

/// Run `iters` ForceAtlas2 iterations on the nodes with `active[v]`.
pub fn run(
    g: &Graph,
    pos: &mut [[f64; 2]],
    active: &[bool],
    iters: u32,
    fp: &ForceParams,
    anchors: Option<&Anchors>,
    initial_speed: f64,
) -> RunStats {
    // Work per iteration ~ nodes (repulsion) + arcs (attraction): dense aggregate
    // graphs (2,791 communities, 7.8M arcs on the 1M benchmark) stay parallel.
    let k = active.iter().filter(|&&a| a).count();
    if k + g.nbr.len() / 16 < SEQUENTIAL_BELOW && rayon::current_num_threads() > 1 {
        if let Ok(pool) = rayon::ThreadPoolBuilder::new().num_threads(1).build() {
            return pool.install(|| run_on_pool(g, pos, active, iters, fp, anchors, initial_speed));
        }
    }
    run_on_pool(g, pos, active, iters, fp, anchors, initial_speed)
}

fn run_on_pool(
    g: &Graph,
    pos: &mut [[f64; 2]],
    active: &[bool],
    iters: u32,
    fp: &ForceParams,
    anchors: Option<&Anchors>,
    initial_speed: f64,
) -> RunStats {
    let t = std::time::Instant::now();
    let n = g.n();
    let members: Vec<u32> = (0..n as u32).filter(|&v| active[v as usize]).collect();
    let mass = &g.mass;
    let mut old = vec![[0.0f64; 2]; n];
    let mut force = vec![[0.0f64; 2]; n];
    let (mut speed, mut efficiency) = (initial_speed, 1.0f64);
    let mut last_step = 0.0;
    if members.len() < 2 {
        return RunStats {
            nodes: members.len(),
            ..Default::default()
        };
    }
    let theta2 = fp.theta * fp.theta;
    let (mut secs_tree, mut secs_repulsion, mut max_leaf) = (0.0, 0.0, 0);
    let mut tree = QuadTree::default();
    let (mut rep, mut steps): (Vec<[f64; 2]>, Vec<f64>) = (Vec::new(), Vec::new());
    for _ in 0..iters {
        let tt = std::time::Instant::now();
        tree.rebuild(pos, mass, &members);
        secs_tree += tt.elapsed().as_secs_f64();
        max_leaf = tree
            .nodes
            .iter()
            .filter(|n| n.count == 0)
            .map(|n| (n.hi - n.lo) as usize)
            .max()
            .unwrap_or(0);
        let tt = std::time::Instant::now();
        // Repulsion in Morton order (cache friendly), scattered back by node.
        (0..tree.keys.len())
            .into_par_iter()
            .with_min_len(CHUNK / 8)
            .map_init(Vec::new, |stack, k| tree.repulsion(k, theta2, stack))
            .collect_into_vec(&mut rep);
        secs_repulsion += tt.elapsed().as_secs_f64();
        for (k, &(_, v)) in tree.keys.iter().enumerate() {
            force[v as usize] = [rep[k][0] * fp.scaling, rep[k][1] * fp.scaling];
        }
        let p: &[[f64; 2]] = pos;
        force
            .par_iter_mut()
            .enumerate()
            .with_min_len(CHUNK)
            .for_each(|(v, f)| {
                if !active[v] {
                    *f = [0.0, 0.0];
                    return;
                }
                let (ns, ws) = g.neighbors(v as u32);
                let [x, y] = p[v];
                for (&u, &w) in ns.iter().zip(ws) {
                    let q = p[u as usize];
                    f[0] += w * (q[0] - x);
                    f[1] += w * (q[1] - y);
                }
                let d = (x * x + y * y).sqrt();
                if d > 0.0 {
                    let gk = if fp.strong_gravity {
                        fp.gravity * mass[v]
                    } else {
                        fp.gravity * mass[v] / d
                    };
                    f[0] -= x * gk;
                    f[1] -= y * gk;
                }
                if let Some(a) = anchors {
                    if a.has[v] {
                        f[0] += fp.anchor * mass[v] * (a.pos[v][0] - x);
                        f[1] += fp.anchor * mass[v] * (a.pos[v][1] - y);
                    }
                }
            });
        // Gephi's adaptive speed.
        let (swinging, traction) = det_sum2(n, |v| {
            if !active[v] {
                return (0.0, 0.0);
            }
            let (f, o) = (force[v], old[v]);
            let sw = ((f[0] - o[0]).powi(2) + (f[1] - o[1]).powi(2)).sqrt();
            let tr = ((f[0] + o[0]).powi(2) + (f[1] + o[1]).powi(2)).sqrt() / 2.0;
            (mass[v] * sw, mass[v] * tr)
        });
        let nn = members.len() as f64;
        let est = 0.05 * nn.sqrt();
        let min_jt = est.sqrt();
        let mut jt = fp.jitter_tolerance * min_jt.max(10f64.min(est * traction / (nn * nn)));
        if swinging / traction.max(1e-300) > 2.0 {
            if efficiency > 0.05 {
                efficiency *= 0.5;
            }
            jt = jt.max(fp.jitter_tolerance);
        }
        let target = jt * efficiency * traction / swinging.max(1e-300);
        if swinging > jt * traction {
            if efficiency > 0.05 {
                efficiency *= 0.7;
            }
        } else if speed < 1000.0 {
            efficiency *= 1.3;
        }
        speed += (target - speed).min(0.5 * speed);
        let s = speed;
        let step_sum: f64 = {
            let f: &[[f64; 2]] = &force;
            let o: &[[f64; 2]] = &old;
            pos.par_iter_mut()
                .enumerate()
                .with_min_len(CHUNK)
                .map(|(v, p)| {
                    if !active[v] {
                        return 0.0;
                    }
                    let sw = mass[v]
                        * ((f[v][0] - o[v][0]).powi(2) + (f[v][1] - o[v][1]).powi(2)).sqrt();
                    let factor = s / (1.0 + (s * sw).sqrt());
                    p[0] += f[v][0] * factor;
                    p[1] += f[v][1] * factor;
                    factor * (f[v][0] * f[v][0] + f[v][1] * f[v][1]).sqrt()
                })
                .collect_into_vec(&mut steps);
            steps.iter().sum()
        };
        last_step = step_sum / nn;
        std::mem::swap(&mut old, &mut force);
    }
    RunStats {
        nodes: members.len(),
        iterations: iters,
        secs: t.elapsed().as_secs_f64(),
        last_step,
        secs_tree,
        secs_repulsion,
        max_leaf,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_cliques_separate_and_run_is_deterministic() {
        let mut e = Vec::new();
        for c in 0..2u32 {
            for i in 0..10 {
                for j in i + 1..10 {
                    e.push((c * 10 + i, c * 10 + j, 1.0));
                }
            }
        }
        e.push((0, 10, 1.0));
        let g = Graph::from_edges(20, &e);
        let init: Vec<[f64; 2]> = (0..20)
            .map(|i| [(i as f64 * 1.7).sin() * 10.0, (i as f64 * 2.3).cos() * 10.0])
            .collect();
        let active = vec![true; 20];
        let mut a = init.clone();
        run(&g, &mut a, &active, 300, &ForceParams::default(), None, 1.0);
        let mut b = init.clone();
        rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| run(&g, &mut b, &active, 300, &ForceParams::default(), None, 1.0));
        assert_eq!(a, b, "bit-identical across thread counts");
        let centroid = |r: std::ops::Range<usize>| {
            let s = a[r.clone()]
                .iter()
                .fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
            [s[0] / r.len() as f64, s[1] / r.len() as f64]
        };
        let (c0, c1) = (centroid(0..10), centroid(10..20));
        let between = ((c0[0] - c1[0]).powi(2) + (c0[1] - c1[1]).powi(2)).sqrt();
        let spread0 = a[0..10]
            .iter()
            .map(|p| ((p[0] - c0[0]).powi(2) + (p[1] - c0[1]).powi(2)).sqrt())
            .sum::<f64>()
            / 10.0;
        assert!(
            between > 2.0 * spread0,
            "cliques should separate: between {between}, spread {spread0}"
        );
    }

    #[test]
    fn quadtree_mass_is_conserved() {
        let pos: Vec<[f64; 2]> = (0..1000)
            .map(|i| [((i * 37) % 101) as f64, ((i * 91) % 103) as f64])
            .collect();
        let mass: Vec<f64> = (0..1000).map(|i| 1.0 + (i % 5) as f64).collect();
        let members: Vec<u32> = (0..1000).collect();
        let t = QuadTree::build(&pos, &mass, &members);
        assert!((t.nodes[0].mass - mass.iter().sum::<f64>()).abs() < 1e-9);
        // θ = 0 is exact: compare one point with brute force.
        let mut st = Vec::new();
        let k = 17;
        let f = t.repulsion(k, 0.0, &mut st);
        let v = t.keys[k].1 as usize;
        let mut b = [0.0, 0.0];
        for u in 0..1000 {
            if u != v {
                let (dx, dy) = (pos[v][0] - pos[u][0], pos[v][1] - pos[u][1]);
                let d2 = dx * dx + dy * dy;
                if d2 > 1e-18 {
                    b[0] += dx * mass[u] / d2 * mass[v];
                    b[1] += dy * mass[u] / d2 * mass[v];
                }
            }
        }
        assert!(
            (f[0] - b[0]).abs() < 1e-6 * b[0].abs().max(1.0)
                && (f[1] - b[1]).abs() < 1e-6 * b[1].abs().max(1.0)
        );
    }
}
