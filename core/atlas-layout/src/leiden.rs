//! Leiden community detection (Traag, Waltman & van Eck 2019, R37): fast local
//! moving, refinement, aggregation by the refined partition, repeated until the
//! partition is stable.  Communities are guaranteed γ-connected.
//!
//! One quality function covers both objectives the paper treats:
//!   H = Σ_c [ E_c − λ S_c² / 2 ]
//! where E_c is the edge weight inside c and S_c the summed node "size":
//!   modularity (resolution γ): size = weighted degree k_v, λ = γ / 2m  (H = m·Q_γ)
//!   CPM (resolution γ):        size = original node count n_v, λ = γ
//! Moving v next to community C gains  w(v, C) − λ s_v S_C.
//!
//! Parallelism: refinement is independent per community (rayon); aggregation is
//! parallel.  Fast local moving is a sequential queue, as in the paper, because a
//! parallel asynchronous version would not be deterministic.  Every random choice
//! comes from a stream keyed by (seed, iteration, level, community).
//!
//! `hierarchy` nests several resolutions: the finest partition is computed on the
//! graph, each coarser one on the aggregate of the previous, so every coarse
//! community is a union of finer ones (area ⊇ region ⊇ neighbourhood).

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::VecDeque;

use crate::graph::Graph;
use crate::util::{Rng, det_sum, group_by, renumber};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    Modularity,
    Cpm,
}

#[derive(Clone, Debug)]
pub struct LeidenParams {
    pub quality: Quality,
    pub resolution: f64,
    pub seed: u64,
    /// Full Leiden runs, each starting from the previous partition; stops early when stable.
    pub iterations: u32,
    /// Randomness of the refinement merge choice (the paper's θ).
    pub theta: f64,
}

impl Default for LeidenParams {
    fn default() -> Self {
        LeidenParams {
            quality: Quality::Modularity,
            resolution: 1.0,
            seed: 42,
            iterations: 2,
            theta: 0.01,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Partition {
    /// Community of each node, renumbered 0..count by first appearance.
    pub membership: Vec<u32>,
    pub count: usize,
}

/// Node sizes and the penalty scale λ for `q` at `resolution`.
fn sizes(g: &Graph, q: Quality, resolution: f64, total_w: f64) -> (Vec<f64>, f64) {
    match q {
        Quality::Modularity => (
            g.strength(),
            if total_w > 0.0 {
                resolution / total_w
            } else {
                0.0
            },
        ),
        Quality::Cpm => (g.node_w.clone(), resolution),
    }
}

pub fn leiden(g: &Graph, p: &LeidenParams, initial: Option<&[u32]>) -> Partition {
    let n = g.n();
    let mut membership: Vec<u32> = match initial {
        Some(m) => {
            assert_eq!(m.len(), n);
            m.to_vec()
        }
        None => (0..n as u32).collect(),
    };
    renumber(&mut membership);
    let total_w: f64 = g.strength().iter().sum();
    for it in 0..p.iterations.max(1) {
        let next = run_once(g, p, total_w, &membership, it as u64);
        let stable = next == membership;
        membership = next;
        if stable {
            break;
        }
    }
    let count = renumber(&mut membership);
    Partition { membership, count }
}

fn run_once(g: &Graph, p: &LeidenParams, total_w: f64, start: &[u32], it: u64) -> Vec<u32> {
    let mut graph: Cow<Graph> = Cow::Borrowed(g);
    let mut node_of: Vec<u32> = (0..g.n() as u32).collect();
    let mut part = start.to_vec();
    for level in 0u64.. {
        let (size, lambda) = sizes(&graph, p.quality, p.resolution, total_w);
        let mut rng = Rng::keyed(&[p.seed, it, level, 0]);
        let tt = std::time::Instant::now();
        fast_local_move(&graph, &size, lambda, &mut part, &mut rng);
        let count = renumber(&mut part);
        if std::env::var_os("ATLAS_LEIDEN_TRACE").is_some() {
            eprintln!(
                "  it {it} level {level}: n={} move {:.3}s -> {count}",
                graph.n(),
                tt.elapsed().as_secs_f64()
            );
        }
        let tt = std::time::Instant::now();
        if count == graph.n() {
            break;
        }
        let mut refined = refine(
            &graph,
            &size,
            lambda,
            &part,
            count,
            p.theta,
            [p.seed, it, level],
        );
        let nr = renumber(&mut refined);
        if std::env::var_os("ATLAS_LEIDEN_TRACE").is_some() {
            eprintln!("    refine {:.3}s -> {nr}", tt.elapsed().as_secs_f64());
        }
        if nr == graph.n() {
            break;
        }
        let mut next_part = vec![0u32; nr];
        for (v, &r) in refined.iter().enumerate() {
            next_part[r as usize] = part[v];
        }
        node_of
            .par_iter_mut()
            .for_each(|x| *x = refined[*x as usize]);
        graph = Cow::Owned(graph.aggregate(&refined, nr));
        part = next_part;
    }
    let mut out: Vec<u32> = node_of.par_iter().map(|&x| part[x as usize]).collect();
    renumber(&mut out);
    out
}

/// Graphs at least this large use the batched parallel local moving.
pub(crate) const PAR_MOVE_MIN: usize = 8_192;
const NO_COMMUNITY: u32 = u32::MAX;

/// Queue-based local moving (paper, Algorithm A.2).  Returns whether anything moved.
fn fast_local_move(g: &Graph, size: &[f64], lambda: f64, m: &mut [u32], rng: &mut Rng) -> bool {
    if g.n() >= PAR_MOVE_MIN {
        // n/16: on the 1M-node benchmark, same objective as n/64 (0.1316) with a
        // quarter of the sync points, 49 s instead of 88 s on a loaded machine.
        let batch = (g.n() / 16).clamp(256, 1 << 16);
        return batched_local_move(g, size, lambda, m, rng, batch);
    }
    sequential_local_move(g, size, lambda, m, rng)
}

/// Where a node goes in one batch of `batched_local_move`.
#[derive(Clone, Copy)]
enum Move {
    Stay,
    To(u32),
    Alone,
}

/// The same queue, taken `batch` nodes at a time: best moves are computed in
/// parallel against the state at the start of the batch, then applied in queue
/// order.  The result depends only on the seed and `batch`, never on the thread
/// count.  Each proposed move is re-checked against the current state with exact
/// weights before it is applied, so, as in the sequential version, every move
/// strictly raises the objective.  Two singletons only merge towards the smaller
/// community id, so adjacent singletons in one batch do not just swap (Lu et al.
/// 2015).
pub(crate) fn batched_local_move(
    g: &Graph,
    size: &[f64],
    lambda: f64,
    m: &mut [u32],
    rng: &mut Rng,
    batch: usize,
) -> bool {
    let n = g.n();
    let mut csize = vec![0.0f64; n];
    let mut ccount = vec![0u32; n];
    for v in 0..n {
        csize[m[v] as usize] += size[v];
        ccount[m[v] as usize] += 1;
    }
    let mut empty: Vec<u32> = (0..n as u32).filter(|&c| ccount[c as usize] == 0).collect();
    let mut order: Vec<u32> = (0..n as u32).collect();
    rng.shuffle(&mut order);
    let mut queue: VecDeque<u32> = order.into();
    let mut queued = vec![true; n];
    let mut moved = false;
    let mut chunk: Vec<u32> = Vec::with_capacity(batch);
    let mut decisions: Vec<Move> = Vec::with_capacity(batch);
    let trace = std::env::var_os("ATLAS_LEIDEN_TRACE").is_some();
    let (mut t_par, mut t_seq, mut visits, mut batches) = (0.0f64, 0.0f64, 0usize, 0usize);
    let (mut arcs_seen, mut arcs_max_batch) = (0usize, 0usize);
    while !queue.is_empty() {
        chunk.clear();
        chunk.extend(queue.drain(..batch.min(queue.len())));
        visits += chunk.len();
        batches += 1;
        if trace {
            let (mut sum, mut mx) = (0usize, 0usize);
            for &v in &chunk {
                let d = g.degree(v);
                sum += d;
                mx = mx.max(d);
            }
            arcs_seen += sum;
            arcs_max_batch += mx;
        }
        let t = std::time::Instant::now();
        for &v in &chunk {
            queued[v as usize] = false;
        }
        let (mm, cs, cc) = (&*m, &csize, &ccount);
        chunk
            .par_iter()
            .with_min_len(256)
            .map(|&v| best_move(g, size, lambda, mm, cs, cc, v))
            .collect_into_vec(&mut decisions);
        t_par += t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        for (&v, &d) in chunk.iter().zip(&decisions) {
            let target = match d {
                Move::Stay => continue,
                Move::To(c) => c,
                Move::Alone => NO_COMMUNITY,
            };
            // Re-check against the state of now with exact weights, so every applied
            // move strictly raises the objective and the loop terminates.
            let cur = m[v as usize];
            let sv = size[v as usize];
            let (ns, ws) = g.neighbors(v);
            let (mut w_to, mut w_cur) = (0.0, 0.0);
            for (&u, &x) in ns.iter().zip(ws) {
                let c = m[u as usize];
                if c == cur {
                    w_cur += x;
                } else if c == target {
                    w_to += x;
                }
            }
            let stay = w_cur - lambda * sv * (csize[cur as usize] - sv);
            let target = if target == NO_COMMUNITY {
                match (stay < 0.0 && ccount[cur as usize] > 1)
                    .then(|| empty.pop())
                    .flatten()
                {
                    Some(e) => e,
                    None => continue,
                }
            } else if target != cur && w_to - lambda * sv * csize[target as usize] > stay {
                target
            } else {
                continue;
            };
            csize[cur as usize] -= sv;
            ccount[cur as usize] -= 1;
            csize[target as usize] += sv;
            ccount[target as usize] += 1;
            m[v as usize] = target;
            moved = true;
            if ccount[cur as usize] == 0 {
                empty.push(cur);
            }
            for &u in ns {
                if !queued[u as usize] && m[u as usize] != target {
                    queued[u as usize] = true;
                    queue.push_back(u);
                }
            }
        }
        t_seq += t.elapsed().as_secs_f64();
    }
    if trace {
        eprintln!(
            "    batched: {visits} visits in {batches} batches, parallel {t_par:.3}s, apply {t_seq:.3}s, arcs {arcs_seen}, sum of per-batch max degree {arcs_max_batch}"
        );
    }
    moved
}

/// Per-thread scratch for `best_move`: a dense accumulator over communities and
/// a list of (community << 32 | neighbour position, weight) arcs.
#[derive(Default)]
struct Scratch {
    w: Vec<f64>,
    touched: Vec<u32>,
    arcs: Vec<(u64, f64)>,
}

thread_local! {
    // ponytail: up to DENSE_MAX_N f64 per rayon thread, kept for the life of the pool.
    static SCRATCH: std::cell::RefCell<Scratch> = std::cell::RefCell::default();
}

/// Graphs up to this many nodes, and nodes with more arcs than `SORT_MAX_DEGREE`,
/// sum community weights in a dense per-thread array; otherwise a node's arcs are
/// sorted by community.  The dense array's random writes miss cache once it
/// outgrows L2 (1M nodes: 8 MB per thread), where sorting ~20 arcs is cheaper;
/// on small aggregate graphs with high degrees the dense array wins (README, Leiden).
const DENSE_MAX_N: usize = 1 << 18;
const SORT_MAX_DEGREE: usize = 1024;

/// Weight from `v` to each neighbouring community, as (community, weight) pairs
/// handed to `f`, each community once, weights summed in neighbour order.
#[inline]
fn community_weights<R>(
    g: &Graph,
    m: &[u32],
    v: u32,
    dense: bool,
    f: impl FnOnce(&mut dyn Iterator<Item = (u32, f64)>) -> R,
) -> R {
    let (ns, ws) = g.neighbors(v);
    SCRATCH.with_borrow_mut(|sc| {
        if dense {
            if sc.w.len() < m.len() {
                sc.w.resize(m.len(), 0.0);
            }
            for (&u, &x) in ns.iter().zip(ws) {
                let c = m[u as usize];
                if sc.w[c as usize] == 0.0 {
                    sc.touched.push(c);
                }
                sc.w[c as usize] += x;
            }
            let Scratch { w, touched, .. } = sc;
            let r = f(&mut touched.iter().map(|&c| (c, w[c as usize])));
            for &c in touched.iter() {
                w[c as usize] = 0.0;
            }
            touched.clear();
            r
        } else {
            let arcs = &mut sc.arcs;
            arcs.clear();
            arcs.extend(
                ns.iter()
                    .zip(ws)
                    .enumerate()
                    .map(|(i, (&u, &x))| ((m[u as usize] as u64) << 32 | i as u64, x)),
            );
            arcs.sort_unstable_by_key(|a| a.0);
            f(&mut arcs
                .chunk_by(|a, b| a.0 >> 32 == b.0 >> 32)
                .map(|run| ((run[0].0 >> 32) as u32, run.iter().map(|a| a.1).sum())))
        }
    })
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn best_move(
    g: &Graph,
    size: &[f64],
    lambda: f64,
    m: &[u32],
    csize: &[f64],
    ccount: &[u32],
    v: u32,
) -> Move {
    if g.degree(v) == 0 {
        return Move::Stay;
    }
    let cur = m[v as usize];
    let sv = size[v as usize];
    let cur_single = ccount[cur as usize] == 1;
    let dense = m.len() <= DENSE_MAX_N || g.degree(v) > SORT_MAX_DEGREE;
    community_weights(g, m, v, dense, |it| {
        // Candidates are compared after the scan: the weight to `cur` may come last.
        let mut w_cur = 0.0;
        let (mut best, mut best_gain) = (cur, f64::NEG_INFINITY);
        for (c, w) in it {
            if c == cur {
                w_cur = w;
                continue;
            }
            if cur_single && ccount[c as usize] == 1 && c > cur {
                continue;
            }
            let gain = w - lambda * sv * csize[c as usize];
            if gain > best_gain || (gain == best_gain && c < best) {
                best = c;
                best_gain = gain;
            }
        }
        let stay_gain = w_cur - lambda * sv * (csize[cur as usize] - sv);
        if best_gain <= stay_gain {
            best = cur;
        }
        if best != cur {
            Move::To(best)
        } else if stay_gain < 0.0 && !cur_single {
            Move::Alone
        } else {
            Move::Stay
        }
    })
}

fn sequential_local_move(
    g: &Graph,
    size: &[f64],
    lambda: f64,
    m: &mut [u32],
    rng: &mut Rng,
) -> bool {
    let n = g.n();
    let mut csize = vec![0.0f64; n];
    let mut ccount = vec![0u32; n];
    for v in 0..n {
        csize[m[v] as usize] += size[v];
        ccount[m[v] as usize] += 1;
    }
    let mut empty: Vec<u32> = (0..n as u32).filter(|&c| ccount[c as usize] == 0).collect();
    let mut order: Vec<u32> = (0..n as u32).collect();
    rng.shuffle(&mut order);
    let mut queue: VecDeque<u32> = order.into();
    let mut queued = vec![true; n];
    let mut wc = vec![0.0f64; n];
    let mut touched: Vec<u32> = Vec::new();
    let mut moved = false;
    while let Some(v) = queue.pop_front() {
        queued[v as usize] = false;
        let (ns, ws) = g.neighbors(v);
        if ns.is_empty() {
            continue;
        }
        let cur = m[v as usize];
        let sv = size[v as usize];
        for (&u, &x) in ns.iter().zip(ws) {
            let c = m[u as usize];
            if wc[c as usize] == 0.0 {
                touched.push(c);
            }
            wc[c as usize] += x;
        }
        csize[cur as usize] -= sv;
        ccount[cur as usize] -= 1;
        let mut best = cur;
        let mut best_gain = wc[cur as usize] - lambda * sv * csize[cur as usize];
        for &c in &touched {
            let gain = wc[c as usize] - lambda * sv * csize[c as usize];
            if gain > best_gain {
                best = c;
                best_gain = gain;
            }
        }
        // Alone in a fresh community gains exactly 0.
        let mut from_empty = false;
        if best_gain < 0.0 && ccount[cur as usize] > 0 {
            if let Some(&e) = empty.last() {
                best = e;
                from_empty = true;
            }
        }
        for &c in &touched {
            wc[c as usize] = 0.0;
        }
        touched.clear();
        csize[best as usize] += sv;
        ccount[best as usize] += 1;
        m[v as usize] = best;
        if best != cur {
            moved = true;
            if from_empty {
                empty.pop();
            }
            if ccount[cur as usize] == 0 {
                empty.push(cur);
            }
            for &u in ns {
                if !queued[u as usize] && m[u as usize] != best {
                    queued[u as usize] = true;
                    queue.push_back(u);
                }
            }
        }
    }
    moved
}

/// Refinement (paper, Algorithm A.3): inside each community, start from
/// singletons and merge well-connected singletons into well-connected refined
/// communities, choosing randomly with probability ∝ exp(ΔH / θ) among
/// non-negative gains.  Returns a refined membership (ids are arbitrary).
fn refine(
    g: &Graph,
    size: &[f64],
    lambda: f64,
    part: &[u32],
    count: usize,
    theta: f64,
    key: [u64; 3],
) -> Vec<u32> {
    let (order, start) = group_by(part, count);
    let mut pos = vec![0u32; g.n()];
    for c in 0..count {
        for (i, &v) in order[start[c]..start[c + 1]].iter().enumerate() {
            pos[v as usize] = i as u32;
        }
    }
    let local: Vec<u32> = (0..count)
        .into_par_iter()
        .flat_map_iter(|c| {
            let members = &order[start[c]..start[c + 1]];
            let mut rng = Rng::keyed(&[key[0], key[1], key[2], c as u64 + 1]);
            refine_one(
                g, size, lambda, part, &pos, c as u32, members, theta, &mut rng,
            )
        })
        .collect();
    // `local` is aligned with `order`; make ids global by offsetting with the community start.
    let mut refined = vec![0u32; g.n()];
    for c in 0..count {
        for i in start[c]..start[c + 1] {
            refined[order[i] as usize] = (start[c] + local[i] as usize) as u32;
        }
    }
    refined
}

#[allow(clippy::too_many_arguments)]
fn refine_one(
    g: &Graph,
    size: &[f64],
    lambda: f64,
    part: &[u32],
    pos: &[u32],
    c: u32,
    members: &[u32],
    theta: f64,
    rng: &mut Rng,
) -> Vec<u32> {
    let k = members.len();
    if k == 1 {
        return vec![0];
    }
    let total: f64 = members.iter().map(|&v| size[v as usize]).sum();
    let mut r: Vec<u32> = (0..k as u32).collect();
    let mut rsize: Vec<f64> = members.iter().map(|&v| size[v as usize]).collect();
    let mut rcount = vec![1u32; k];
    // ext[x]: weight from refined community x to the rest of c.
    let mut ext: Vec<f64> = members
        .iter()
        .map(|&v| {
            let (ns, ws) = g.neighbors(v);
            ns.iter()
                .zip(ws)
                .filter(|(u, _)| part[**u as usize] == c)
                .map(|(_, x)| x)
                .sum()
        })
        .collect();
    let mut visit: Vec<u32> = (0..k as u32).collect();
    rng.shuffle(&mut visit);
    let mut wc = vec![0.0f64; k];
    let mut touched: Vec<u32> = Vec::new();
    let mut cand: Vec<(u32, f64)> = Vec::new();
    for &i in &visit {
        let own = r[i as usize];
        if rcount[own as usize] != 1 {
            continue; // only singletons move
        }
        let v = members[i as usize];
        let sv = size[v as usize];
        if ext[own as usize] < lambda * sv * (total - sv) {
            continue; // v is not well connected to its community
        }
        let (ns, ws) = g.neighbors(v);
        for (&u, &x) in ns.iter().zip(ws) {
            if part[u as usize] != c {
                continue;
            }
            let rc = r[pos[u as usize] as usize];
            if wc[rc as usize] == 0.0 {
                touched.push(rc);
            }
            wc[rc as usize] += x;
        }
        cand.clear();
        cand.push((own, 0.0));
        for &rc in &touched {
            if rc == own {
                continue;
            }
            let s = rsize[rc as usize];
            if ext[rc as usize] < lambda * s * (total - s) {
                continue; // target not well connected
            }
            let gain = wc[rc as usize] - lambda * sv * s;
            if gain >= 0.0 {
                cand.push((rc, gain));
            }
        }
        let chosen = choose(&cand, theta, rng);
        if chosen != own {
            ext[chosen as usize] += ext[own as usize] - 2.0 * wc[chosen as usize];
            rsize[chosen as usize] += sv;
            rcount[chosen as usize] += 1;
            rcount[own as usize] = 0;
            r[i as usize] = chosen;
        }
        for &rc in &touched {
            wc[rc as usize] = 0.0;
        }
        touched.clear();
    }
    r
}

fn choose(cand: &[(u32, f64)], theta: f64, rng: &mut Rng) -> u32 {
    if cand.len() == 1 {
        return cand[0].0;
    }
    let max = cand.iter().map(|c| c.1).fold(f64::NEG_INFINITY, f64::max);
    let total: f64 = cand.iter().map(|c| ((c.1 - max) / theta).exp()).sum();
    let mut x = rng.unit() * total;
    for &(id, gain) in cand {
        x -= ((gain - max) / theta).exp();
        if x <= 0.0 {
            return id;
        }
    }
    cand[cand.len() - 1].0
}

/// Quality of a partition: standard modularity (γ = 1) and the objective at
/// `resolution` (Q_γ for modularity; H = Σ_c [E_c − γ n_c² / 2] for CPM).
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Scores {
    pub modularity: f64,
    pub objective: f64,
}

pub fn scores(g: &Graph, membership: &[u32], count: usize, q: Quality, resolution: f64) -> Scores {
    let k = g.strength();
    let two_m: f64 = det_sum(g.n(), |v| k[v]);
    let internal: Vec<f64> = (0..g.n())
        .into_par_iter()
        .map(|v| {
            let (ns, ws) = g.neighbors(v as u32);
            let c = membership[v];
            ns.iter()
                .zip(ws)
                .filter(|(u, _)| membership[**u as usize] == c)
                .map(|(_, x)| x)
                .sum::<f64>()
                + 2.0 * g.self_w[v]
        })
        .collect();
    let (mut e, mut kc, mut nc) = (
        vec![0.0f64; count],
        vec![0.0f64; count],
        vec![0.0f64; count],
    );
    for v in 0..g.n() {
        let c = membership[v] as usize;
        e[c] += internal[v] / 2.0;
        kc[c] += k[v];
        nc[c] += g.node_w[v];
    }
    let modq = |gamma: f64| {
        if two_m == 0.0 {
            return 0.0;
        }
        (0..count)
            .map(|c| 2.0 * e[c] / two_m - gamma * (kc[c] / two_m).powi(2))
            .sum::<f64>()
    };
    let objective = match q {
        Quality::Modularity => modq(resolution),
        Quality::Cpm => (0..count)
            .map(|c| e[c] - resolution * nc[c] * nc[c] / 2.0)
            .sum(),
    };
    Scores {
        modularity: modq(1.0),
        objective,
    }
}

/// One level of `hierarchy`.
#[derive(Clone, Debug)]
pub struct Level {
    pub partition: Partition,
    /// Leiden time, bisection included.
    pub secs: f64,
    /// The resolution used: the one asked for, or the one `max_share` led to.
    pub resolution: f64,
}

/// Bisection steps (in log γ) when a coarse level breaks `max_share`.
const SHARE_STEPS: u32 = 10;

/// Nested partitions of the base nodes, one per resolution, returned in the order
/// given (coarse to fine means resolutions increasing for modularity and CPM).
/// The finest is computed first on `g`; each coarser one on the aggregate graph.
pub fn hierarchy(g: &Graph, resolutions: &[f64], base: &LeidenParams) -> Vec<Level> {
    hierarchy_seeded(g, resolutions, base, &[], &[], 1.0)
}

/// `hierarchy`, warm-started: `seeds[l]`, when present, gives a previous community
/// label per base node for level `l` (`u32::MAX` = none).  Each graph node of that
/// level starts in the label most of its base members carry; nodes with no label
/// start alone.  Leiden then only moves what the new graph asks to move.
///
/// `max_share` caps a coarse level's largest community (as a share of the nodes
/// that have edges).  At low resolution modularity folds a whole connected
/// component into one community (the resolution limit), which makes a useless top
/// level of the map.  A coarse level over the cap is recomputed at the smallest
/// resolution, between the one asked for and the next finer level's, that meets
/// the cap (bisection on the small aggregate graph); if even the finer resolution
/// does not, it keeps that one.  The finest level is never changed.  `hints[l]`,
/// a previous build's resolution for level `l`, is tried first and kept if it
/// still meets the cap, so a warm rebuild does not drift to another resolution.
pub fn hierarchy_seeded(
    g: &Graph,
    resolutions: &[f64],
    base: &LeidenParams,
    seeds: &[Option<Vec<u32>>],
    hints: &[Option<f64>],
    max_share: f64,
) -> Vec<Level> {
    let mut order: Vec<usize> = (0..resolutions.len()).collect();
    // finest (largest resolution) first
    order.sort_by(|&a, &b| resolutions[b].total_cmp(&resolutions[a]));
    let mut out: Vec<Option<Level>> = vec![None; resolutions.len()];
    let mut graph: Cow<Graph> = Cow::Borrowed(g);
    let mut node_of: Vec<u32> = (0..g.n() as u32).collect();
    let mut finer: Option<f64> = None;
    for (step, &li) in order.iter().enumerate() {
        let p = LeidenParams {
            resolution: resolutions[li],
            seed: base.seed ^ (step as u64 * 0x9E37),
            ..base.clone()
        };
        let t = std::time::Instant::now();
        let initial = seeds
            .get(li)
            .and_then(|s| s.as_deref())
            .map(|s| majority_labels(&node_of, graph.n(), s));
        let run = |r: f64| {
            leiden(
                &graph,
                &LeidenParams {
                    resolution: r,
                    ..p.clone()
                },
                initial.as_deref(),
            )
        };
        let (part, resolution) = match finer {
            Some(hi) if max_share < 1.0 => {
                let hint = hints.get(li).copied().flatten();
                capped(&graph, p.resolution, hi, max_share, hint, run)
            }
            _ => (run(p.resolution), p.resolution),
        };
        let secs = t.elapsed().as_secs_f64();
        // Aggregate nodes are indexed by `part` ids, so the map base node -> next
        // graph node is exactly the (not yet renumbered) base membership.
        node_of = node_of
            .par_iter()
            .map(|&x| part.membership[x as usize])
            .collect();
        graph = Cow::Owned(graph.aggregate(&part.membership, part.count));
        let mut membership = node_of.clone();
        let count = renumber(&mut membership);
        out[li] = Some(Level {
            partition: Partition { membership, count },
            secs,
            resolution,
        });
        finer = Some(resolution);
    }
    out.into_iter()
        .map(|p| p.expect("every level computed"))
        .collect()
}

/// Largest community's share of the node weight of nodes with edges.
fn largest_share(g: &Graph, p: &Partition) -> f64 {
    let mut w = vec![0.0f64; p.count];
    let mut total = 0.0;
    for v in 0..g.n() {
        if g.degree(v as u32) > 0 || g.self_w[v] > 0.0 {
            w[p.membership[v] as usize] += g.node_w[v];
            total += g.node_w[v];
        }
    }
    if total == 0.0 {
        0.0
    } else {
        w.iter().fold(0.0f64, |a, &b| a.max(b)) / total
    }
}

/// The partition at the smallest resolution in [lo, hi] whose largest share is at
/// most `cap` (bisection in log space, assuming the share falls as γ grows); `hi`'s
/// partition if none does.
fn capped(
    g: &Graph,
    lo: f64,
    hi: f64,
    cap: f64,
    hint: Option<f64>,
    run: impl Fn(f64) -> Partition,
) -> (Partition, f64) {
    if let Some(h) = hint.filter(|h| (lo..=hi).contains(h)) {
        let p = run(h);
        if largest_share(g, &p) <= cap {
            return (p, h);
        }
    }
    let first = run(lo);
    if largest_share(g, &first) <= cap || hi <= lo {
        return (first, lo);
    }
    let mut best = (run(hi), hi);
    if largest_share(g, &best.0) > cap {
        return best;
    }
    let (mut a, mut b) = (lo.ln(), hi.ln());
    for _ in 0..SHARE_STEPS {
        let mid = (a + b) / 2.0;
        let p = run(mid.exp());
        if largest_share(g, &p) <= cap {
            best = (p, mid.exp());
            b = mid;
        } else {
            a = mid;
        }
    }
    best
}

/// For each of `n` graph nodes, the most common label among the base nodes mapped
/// to it (`node_of`), ties to the smaller label; unlabeled nodes get fresh ids.
fn majority_labels(node_of: &[u32], n: usize, labels: &[u32]) -> Vec<u32> {
    let mut pairs: Vec<(u32, u32)> = node_of
        .iter()
        .zip(labels)
        .filter(|(_, l)| **l != u32::MAX)
        .map(|(&x, &l)| (x, l))
        .collect();
    pairs.par_sort_unstable();
    let next = labels
        .iter()
        .filter(|&&l| l != u32::MAX)
        .max()
        .map_or(0, |&l| l + 1);
    let mut out: Vec<u32> = (0..n as u32).map(|x| next + x).collect();
    let mut best = vec![0usize; n];
    for run in pairs.chunk_by(|a, b| a == b) {
        let (x, l) = run[0];
        if run.len() > best[x as usize] {
            best[x as usize] = run.len();
            out[x as usize] = l;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring_of_cliques(cliques: u32, k: u32) -> Graph {
        let mut e = Vec::new();
        for c in 0..cliques {
            for i in 0..k {
                for j in i + 1..k {
                    e.push((c * k + i, c * k + j, 1.0));
                }
            }
            e.push((c * k, ((c + 1) % cliques) * k + 1, 1.0));
        }
        Graph::from_edges((cliques * k) as usize, &e)
    }

    #[test]
    fn cpm_recovers_ring_of_cliques() {
        let g = ring_of_cliques(30, 5);
        let p = leiden(
            &g,
            &LeidenParams {
                quality: Quality::Cpm,
                resolution: 0.5,
                ..Default::default()
            },
            None,
        );
        assert_eq!(p.count, 30);
        for c in 0..30usize {
            let m = &p.membership[c * 5..c * 5 + 5];
            assert!(m.iter().all(|&x| x == m[0]), "clique {c} split: {m:?}");
        }
    }

    #[test]
    fn batched_moving_recovers_cliques_for_any_thread_count() {
        let g = ring_of_cliques(2000, 5);
        assert!(g.n() >= PAR_MOVE_MIN, "must exercise the batched path");
        let p = LeidenParams {
            quality: Quality::Cpm,
            resolution: 0.5,
            iterations: 1,
            ..Default::default()
        };
        let a = leiden(&g, &p, None);
        assert_eq!(a.count, 2000);
        assert!(a.membership.chunks(5).all(|m| m.iter().all(|&x| x == m[0])));
        let b = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| leiden(&g, &p, None));
        assert_eq!(a, b);
    }

    #[test]
    fn dense_and_sorted_community_weights_agree() {
        let mut rng = Rng::keyed(&[3]);
        let e: Vec<(u32, u32, f64)> = (0..5000)
            .map(|_| {
                (
                    rng.below(500) as u32,
                    rng.below(500) as u32,
                    0.1 + rng.unit(),
                )
            })
            .collect();
        let g = Graph::from_edges(500, &e);
        let m: Vec<u32> = (0..500).map(|_| rng.below(40) as u32).collect();
        for v in 0..500u32 {
            let get = |dense| {
                let mut out: Vec<(u32, u64)> = community_weights(&g, &m, v, dense, |it| {
                    it.map(|(c, w)| (c, w.to_bits())).collect()
                });
                out.sort_unstable();
                out
            };
            assert_eq!(get(true), get(false), "node {v}");
        }
    }

    #[test]
    fn deterministic_for_a_seed() {
        let g = ring_of_cliques(40, 6);
        let p = LeidenParams::default();
        let a = leiden(&g, &p, None);
        let b = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| leiden(&g, &p, None));
        assert_eq!(a, b);
    }
}
