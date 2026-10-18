//! Multilevel layout: the coarsest community graph is laid out first, then each
//! finer level starts from its parent's position and is refined, down to the
//! nodes themselves (the classic coarsen / interpolate / refine scheme, with the
//! Leiden hierarchy as the coarsening).
//!
//! Warm start: when a previous layout exists, nodes it knew start where they were
//! and are held there by an anchor force; new nodes start at the mean of their
//! known neighbours (else of their community).  Only the finest level is refined,
//! so existing nodes barely move.

use serde::{Deserialize, Serialize};

use crate::force::{self, Anchors, ForceParams, RunStats};
use crate::graph::Graph;
use crate::util::{Rng, group_by, mix};

const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;
/// A parent's children are spread over a disk of radius
/// `SPREAD · R · sqrt(mass_parent / mass_total)`, R being the RMS radius of the parent layout.
const SPREAD: f64 = 0.5;
/// Gap between the map and the disk of unknown nodes, as a share of the map's side.
const UNKNOWN_GAP: f64 = 0.05;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelRun {
    /// "L<k>" for community levels, "nodes" for the base graph.
    pub level: String,
    pub stats: RunStats,
}

/// Cold multilevel layout.  `levels` are base-node memberships, coarse to fine.
pub fn cold(
    g: &Graph,
    levels: &[(Vec<u32>, usize)],
    active: &[bool],
    iterations: u32,
    refine_iterations: u32,
    fp: &ForceParams,
    seed: u64,
) -> (Vec<[f64; 2]>, Vec<LevelRun>) {
    let mut runs = Vec::new();
    let mut prev: Option<(Vec<[f64; 2]>, Vec<f64>)> = None;
    for (l, (m, count)) in levels.iter().enumerate() {
        let agg = g.aggregate(m, *count);
        let act: Vec<bool> = agg.mass.iter().map(|&x| x > 0.0).collect();
        let mut pos = match &prev {
            None => sunflower(&agg.mass, &act, seed),
            Some((ppos, pmass)) => {
                // parent of community c at level l = its members' community at level l-1
                let mut parent = vec![0u32; *count];
                for (v, &c) in m.iter().enumerate() {
                    parent[c as usize] = levels[l - 1].0[v];
                }
                expand(ppos, pmass, &parent, &agg.mass, &act, seed ^ l as u64)
            }
        };
        let iters = if prev.is_none() {
            iterations
        } else {
            refine_iterations
        };
        let stats = force::run(&agg, &mut pos, &act, iters, fp, None, 1.0);
        runs.push(LevelRun {
            level: format!("L{l}"),
            stats,
        });
        prev = Some((pos, agg.mass));
    }
    let mut pos = match (&prev, levels.last()) {
        (Some((ppos, pmass)), Some((m, _))) => {
            expand(ppos, pmass, m, &g.mass, active, seed ^ 0xBA5E)
        }
        _ => sunflower(&g.mass, active, seed),
    };
    let iters = if prev.is_none() {
        iterations
    } else {
        refine_iterations
    };
    let stats = force::run(g, &mut pos, active, iters, fp, None, 1.0);
    runs.push(LevelRun {
        level: "nodes".into(),
        stats,
    });
    (pos, runs)
}

/// Warm start from `prior` (layout coordinates; `has[v]` where known).
/// `fine` is the finest community membership, used to place new nodes without known neighbours.
#[allow(clippy::too_many_arguments)]
pub fn warm(
    g: &Graph,
    active: &[bool],
    prior: &[[f64; 2]],
    has: &[bool],
    fine: Option<&(Vec<u32>, usize)>,
    iterations: u32,
    fp: &ForceParams,
    seed: u64,
) -> (Vec<[f64; 2]>, Vec<LevelRun>) {
    let n = g.n();
    let mut pos = prior.to_vec();
    // Fallback anchors: community centroid of known members, then global centroid.
    let known: Vec<usize> = (0..n).filter(|&v| has[v] && active[v]).collect();
    let global = centroid(known.iter().map(|&v| prior[v]));
    let comm = fine.map(|(m, count)| {
        let mut s = vec![[0.0f64, 0.0, 0.0]; *count];
        for &v in &known {
            let c = m[v] as usize;
            s[c] = [s[c][0] + prior[v][0], s[c][1] + prior[v][1], s[c][2] + 1.0];
        }
        s
    });
    let scale = rms_radius(&known.iter().map(|&v| prior[v]).collect::<Vec<_>>(), global)
        / (n.max(1) as f64).sqrt();
    for v in 0..n {
        if has[v] || !active[v] {
            continue;
        }
        let (ns, _) = g.neighbors(v as u32);
        let mean = centroid(
            ns.iter()
                .filter(|&&u| has[u as usize])
                .map(|&u| prior[u as usize]),
        );
        let base = mean
            .or_else(|| {
                let (m, _) = fine?;
                let s = comm.as_ref()?[m[v] as usize];
                (s[2] > 0.0).then(|| [s[0] / s[2], s[1] / s[2]])
            })
            .or(global)
            .unwrap_or([0.0, 0.0]);
        let h = mix(seed ^ v as u64);
        let a = (h & 0xFFFF) as f64 / 65536.0 * std::f64::consts::TAU;
        pos[v] = [base[0] + scale * a.cos(), base[1] + scale * a.sin()];
    }
    let anchors = Anchors { pos: prior, has };
    // Start slow: the prior is already near equilibrium.
    let stats = force::run(g, &mut pos, active, iterations, fp, Some(&anchors), 0.1);
    (
        pos,
        vec![LevelRun {
            level: "nodes (warm)".into(),
            stats,
        }],
    )
}

/// Nodes with no relation in the layers have nothing to place them, so they sit
/// apart: on a sunflower disk right of the map, one slot per node, the slot area
/// matching the map's mean area per node.  Slot i lies at radius sqrt(c (i + ½)),
/// so a disk grows outwards without moving earlier slots.  Nodes with `kept[v]`
/// keep `pos[v]` (a warm start); new ones take the next slots of the kept disk.
pub fn place_unknown(
    pos: &mut [[f64; 2]],
    placed: &[bool],
    unknown: &[u32],
    kept: &[bool],
    seed: u64,
) {
    let (old, new): (Vec<u32>, Vec<u32>) = unknown.iter().partition(|&&v| kept[v as usize]);
    if new.is_empty() {
        return;
    }
    let (center, c, first) = match centroid(old.iter().map(|&v| pos[v as usize])) {
        Some(ctr) => {
            let r2 = old
                .iter()
                .map(|&v| {
                    (pos[v as usize][0] - ctr[0]).powi(2) + (pos[v as usize][1] - ctr[1]).powi(2)
                })
                .fold(0.0f64, f64::max);
            (ctr, (r2 / old.len() as f64).max(1e-12), old.len())
        }
        None => {
            let (mut x0, mut y0, mut x1, mut y1, mut k) =
                (f64::MAX, f64::MAX, f64::MIN, f64::MIN, 0usize);
            for (p, _) in pos.iter().zip(placed).filter(|(_, a)| **a) {
                (x0, y0, x1, y1, k) = (
                    x0.min(p[0]),
                    y0.min(p[1]),
                    x1.max(p[0]),
                    y1.max(p[1]),
                    k + 1,
                );
            }
            if k == 0 {
                (x0, y0, x1, y1, k) = (0.0, 0.0, 1.0, 1.0, 1);
            }
            let side = (x1 - x0).max(y1 - y0).max(1e-9);
            let c = ((x1 - x0).max(side * 0.01) * (y1 - y0).max(side * 0.01)
                / (std::f64::consts::PI * k as f64))
                .max(1e-12);
            let r = (c * new.len() as f64).sqrt();
            ([x1 + UNKNOWN_GAP * side + r, (y0 + y1) / 2.0], c, 0)
        }
    };
    let phase = Rng::keyed(&[seed, 11]).unit() * std::f64::consts::TAU;
    for (j, &v) in new.iter().enumerate() {
        let i = (first + j) as f64;
        let (r, a) = ((c * (i + 0.5)).sqrt(), i * GOLDEN_ANGLE + phase);
        pos[v as usize] = [center[0] + r * a.cos(), center[1] + r * a.sin()];
    }
}

fn centroid(it: impl Iterator<Item = [f64; 2]>) -> Option<[f64; 2]> {
    let (mut s, mut k) = ([0.0, 0.0], 0usize);
    for p in it {
        s[0] += p[0];
        s[1] += p[1];
        k += 1;
    }
    (k > 0).then(|| [s[0] / k as f64, s[1] / k as f64])
}

fn rms_radius(pts: &[[f64; 2]], c: Option<[f64; 2]>) -> f64 {
    let Some(c) = c else { return 1.0 };
    if pts.is_empty() {
        return 1.0;
    }
    (pts.iter()
        .map(|p| (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2))
        .sum::<f64>()
        / pts.len() as f64)
        .sqrt()
        .max(1e-9)
}

/// Initial positions: a sunflower spiral, heaviest nodes at the centre.
fn sunflower(mass: &[f64], active: &[bool], seed: u64) -> Vec<[f64; 2]> {
    let mut order: Vec<u32> = (0..mass.len() as u32)
        .filter(|&v| active[v as usize])
        .collect();
    order.sort_by(|&a, &b| {
        mass[b as usize]
            .total_cmp(&mass[a as usize])
            .then(a.cmp(&b))
    });
    let total: f64 = order.iter().map(|&v| mass[v as usize]).sum();
    let radius = total.sqrt().max(1.0);
    let phase = Rng::keyed(&[seed, 7]).unit() * std::f64::consts::TAU;
    let mut pos = vec![[0.0f64; 2]; mass.len()];
    let k = order.len().max(1) as f64;
    for (i, &v) in order.iter().enumerate() {
        let r = radius * ((i as f64 + 0.5) / k).sqrt();
        let a = i as f64 * GOLDEN_ANGLE + phase;
        pos[v as usize] = [r * a.cos(), r * a.sin()];
    }
    pos
}

/// Children start on a sunflower inside their parent's disk.
fn expand(
    ppos: &[[f64; 2]],
    pmass: &[f64],
    parent: &[u32],
    mass: &[f64],
    active: &[bool],
    seed: u64,
) -> Vec<[f64; 2]> {
    let np = ppos.len();
    let total: f64 = pmass.iter().sum::<f64>().max(1e-300);
    let c = {
        let s = (0..np).fold([0.0, 0.0], |s, p| {
            [s[0] + pmass[p] * ppos[p][0], s[1] + pmass[p] * ppos[p][1]]
        });
        [s[0] / total, s[1] / total]
    };
    let r_all = ((0..np)
        .map(|p| pmass[p] * ((ppos[p][0] - c[0]).powi(2) + (ppos[p][1] - c[1]).powi(2)))
        .sum::<f64>()
        / total)
        .sqrt()
        .max(1.0);
    let (order, start) = group_by(parent, np);
    let mut pos = vec![[0.0f64; 2]; parent.len()];
    let mut kids: Vec<u32> = Vec::new();
    for p in 0..np {
        kids.clear();
        kids.extend(
            order[start[p]..start[p + 1]]
                .iter()
                .copied()
                .filter(|&v| active[v as usize]),
        );
        kids.sort_by(|&a, &b| {
            mass[b as usize]
                .total_cmp(&mass[a as usize])
                .then(a.cmp(&b))
        });
        let r = SPREAD * r_all * (pmass[p] / total).sqrt();
        let phase =
            (mix(seed ^ p as u64) >> 11) as f64 / (1u64 << 53) as f64 * std::f64::consts::TAU;
        let k = kids.len().max(1) as f64;
        for &v in &order[start[p]..start[p + 1]] {
            pos[v as usize] = ppos[p];
        }
        for (i, &v) in kids.iter().enumerate() {
            let rr = r * ((i as f64 + 0.5) / k).sqrt();
            let a = i as f64 * GOLDEN_ANGLE + phase;
            pos[v as usize] = [ppos[p][0] + rr * a.cos(), ppos[p][1] + rr * a.sin()];
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_disk_sits_right_of_the_map_and_grows_outwards() {
        // 100 placed nodes in [0, 10]^2, then 30 unknown ones.
        let mut pos: Vec<[f64; 2]> = (0..130)
            .map(|i| [(i % 10) as f64 + 0.5, (i / 10 % 10) as f64 + 0.5])
            .collect();
        let placed: Vec<bool> = (0..130).map(|i| i < 100).collect();
        let unknown: Vec<u32> = (100..130).collect();
        place_unknown(&mut pos, &placed, &unknown, &[false; 130], 1);
        assert!(
            unknown.iter().all(|&v| pos[v as usize][0] > 9.5),
            "disk overlaps the map"
        );
        // Warm: the 30 keep their place, 10 more take outer slots.
        let mut pos2 = pos.clone();
        pos2.extend((0..10).map(|_| [0.0, 0.0]));
        let mut placed2 = placed.clone();
        placed2.extend([false; 10]);
        let kept: Vec<bool> = (0..140).map(|v| (100..130).contains(&v)).collect();
        let all: Vec<u32> = (100..140).collect();
        place_unknown(&mut pos2, &placed2, &all, &kept, 1);
        assert_eq!(pos2[..130], pos[..]);
        let c = centroid((100..130).map(|v| pos[v])).unwrap();
        let d = |p: [f64; 2]| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt();
        let rmax = (100..130).map(|v| d(pos[v])).fold(0.0, f64::max);
        assert!(
            (130..140).all(|v| d(pos2[v]) > rmax),
            "new unknown nodes overlap kept ones"
        );
    }
}
