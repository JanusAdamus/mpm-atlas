//! `compute_with` and `tile`: the steps between the snapshot and the files.

use atlas_core::hashidx::key_hash;
use atlas_core::{
    Error, EvidenceState, NONE, Projection, RELATIONS, RelationType, Result, Snapshot,
};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::time::Instant;

use crate::concept;
use crate::graph::Graph;
use crate::leiden::{self, LeidenParams};
use crate::multilevel;
use crate::tiles::{self, Bytes, TileMeta, TileSet};
use crate::util::{det_sum2, group_by};
use crate::{
    Aggregate, ComputeOptions, Layout, LayoutParams, LayoutStats, LevelInfo, TileEdge, TileNode,
    TileNodeKind, TileResponse, WarmStats,
};

/// A warm start needs at least this share of active nodes with a previous position.
const WARM_MIN_SHARE: f64 = 0.5;
const MARGIN: f64 = 0.02;
/// Part of the layout version: a change in how positions or tiles are computed must
/// change the version (it keys warm starts, ETags and client caches).
const ALGORITHM: &str = "atlas-layout/concept-layer-4";

fn validate(p: &LayoutParams) -> Result<()> {
    let bad = |m: &str| Err(Error::BadRequest(format!("layout params: {m}")));
    if p.resolutions.is_empty() || p.resolutions.iter().any(|r| !(r.is_finite() && *r > 0.0)) {
        return bad("resolutions must be non-empty, finite and > 0");
    }
    if !(p.max_share > 0.0 && p.max_share <= 1.0) {
        return bad("max_share must be in (0, 1]");
    }
    if p.layers.is_empty() {
        return bad("layers must name at least one projection");
    }
    let f = &p.force;
    if [f.theta, f.scaling, f.gravity, f.jitter_tolerance, f.anchor]
        .iter()
        .any(|x| !(x.is_finite() && *x >= 0.0))
    {
        return bad("force parameters must be finite and >= 0");
    }
    let t = &p.tiles;
    if t.entities == 0 || t.clusters == 0 || t.edges == 0 || t.full == 0 || t.max_zoom > 20 {
        return bad("tile budgets must be > 0 and max_zoom <= 20");
    }
    if t.general_nodes == 0
        || t.general_edges == 0
        || t.detail_zoom <= t.general_zoom
        || t.detail_zoom > 20
    {
        return bad(
            "general_nodes and general_edges must be > 0 and general_zoom < detail_zoom <= 20",
        );
    }
    Ok(())
}

/// Relations of `layers` that pull nodes together.  `member_of_concept` is not one:
/// it is the grouping the map draws around each concept (`concept`), and the force
/// layout runs on the same graph as without the concept layer, so adding the
/// projection does not move the map.
pub(crate) fn relation_mask(layers: &[Projection]) -> u32 {
    RELATIONS
        .iter()
        .filter(|s| layers.contains(&s.projection))
        .filter(|s| s.relation != RelationType::MemberOfConcept)
        .fold(0, |m, s| m | 1 << s.relation.code())
}

pub(crate) fn run(snap: &Snapshot, params: &LayoutParams, opts: &ComputeOptions) -> Result<Layout> {
    validate(params)?;
    let t0 = Instant::now();
    let n = snap.num_nodes() as usize;
    let merged: Vec<bool> = (0..n as u32)
        .into_par_iter()
        .map(|v| snap.is_merged(v))
        .collect();
    let mut active: Vec<bool> = merged.iter().map(|m| !m).collect();
    if let Some(ex) = &opts.exclude {
        if ex.len() != n {
            return Err(Error::BadRequest(format!(
                "exclude has {} entries, snapshot has {n} nodes",
                ex.len()
            )));
        }
        active.iter_mut().zip(ex).for_each(|(a, &e)| *a &= !e);
    }
    let g = Graph::from_snapshot(snap, relation_mask(&params.layers), &active);
    let secs_graph = t0.elapsed().as_secs_f64();

    let t = Instant::now();
    let lp = LeidenParams {
        quality: params.quality,
        resolution: 1.0,
        seed: params.seed,
        iterations: params.leiden_iterations,
        theta: 0.01,
    };
    let concept_of = concept::primary_concepts(snap, &active);
    // Concepts with members but no relation in the layers (derived ones) sit at
    // their members' centroid; they are not "unknown".
    let hub: Vec<bool> = (0..n)
        .map(|v| active[v] && g.degree(v as u32) == 0 && !snap.concept_members(v as u32).is_empty())
        .collect();
    let unknown_flag: Vec<bool> = (0..n)
        .map(|v| !merged[v] && !hub[v] && (!active[v] || g.degree(v as u32) == 0))
        .collect();
    let id_hash: Vec<u64> = (0..n as u32)
        .into_par_iter()
        .map(|v| key_hash(snap.node_id(v).as_bytes()))
        .collect();
    // Previous node of each active node (NONE if new); used only if enough match.
    let prior = opts.prior.as_ref().and_then(|p| {
        let idx: Vec<u32> = (0..n)
            .into_par_iter()
            .map(|v| {
                if active[v] {
                    p.map.get(&id_hash[v]).copied().unwrap_or(NONE)
                } else {
                    NONE
                }
            })
            .collect();
        let matched = idx.iter().filter(|&&i| i != NONE).count();
        let total = active.iter().filter(|&&a| a).count();
        (matched as f64 >= WARM_MIN_SHARE * total as f64 && matched > 0)
            .then_some((p, idx, matched, total))
    });
    // Leiden seeds: previous community per node, for levels whose resolution is unchanged.
    let seeds: Vec<Option<Vec<u32>>> = params
        .resolutions
        .iter()
        .map(|&r| {
            let (p, idx, ..) = prior.as_ref()?;
            let (_, _, m) = p.levels.iter().find(|(pr, ..)| *pr == r)?;
            Some(
                (0..n)
                    .into_par_iter()
                    .map(|v| {
                        if idx[v] == NONE || unknown_flag[v] {
                            NONE
                        } else {
                            m[idx[v] as usize]
                        }
                    })
                    .collect(),
            )
        })
        .collect();
    // The resolution each level used last time (0 in layouts written before it was kept).
    let hints: Vec<Option<f64>> = params
        .resolutions
        .iter()
        .map(|&r| {
            let (p, ..) = prior.as_ref()?;
            p.levels
                .iter()
                .find(|(pr, ..)| *pr == r)
                .map(|l| l.1)
                .filter(|&u| u > 0.0)
        })
        .collect();
    let parts = leiden::hierarchy_seeded(
        &g,
        &params.resolutions,
        &lp,
        &seeds,
        &hints,
        params.max_share,
    );
    let mut levels = Vec::new();
    let mut infos = Vec::new();
    for (l, level) in parts.into_iter().enumerate() {
        let (m, count, unknown, sizes) = postprocess(
            snap,
            &active,
            &unknown_flag,
            &merged,
            level.partition.membership,
        );
        let sc = leiden::scores(&g, &m, count, params.quality, level.resolution);
        let mut sorted: Vec<u32> = sizes
            .iter()
            .enumerate()
            .filter(|(c, _)| Some(*c as u32) != unknown)
            .map(|(_, &s)| s)
            .collect();
        sorted.sort_unstable();
        infos.push(LevelInfo {
            resolution: params.resolutions[l],
            resolution_used: level.resolution,
            count,
            modularity: sc.modularity,
            objective: sc.objective,
            unknown,
            largest: sorted.last().copied().unwrap_or(0),
            median_size: sorted.get(sorted.len() / 2).copied().unwrap_or(0),
            singletons: sorted.iter().filter(|&&s| s == 1).count(),
            leiden_secs: level.secs,
            sizes,
            centroids: Vec::new(),
            central: Vec::new(),
        });
        levels.push((m, count));
    }
    let secs_leiden = t.elapsed().as_secs_f64();

    let t = Instant::now();
    let warm = prior.as_ref().map(|(p, idx, matched, total)| WarmStats {
        prior_version: p.version.clone(),
        matched: *matched,
        new: total - matched,
        communities_kept: seeds
            .iter()
            .zip(&levels)
            .map(|(s, (m, count))| s.as_ref().map(|s| kept_share(s, idx, m, *count)))
            .collect(),
    });
    // Unknown nodes stay out of the force layout and are placed apart afterwards.
    let placed: Vec<bool> = (0..n)
        .map(|v| active[v] && !unknown_flag[v] && !hub[v])
        .collect();
    let unknown_nodes: Vec<u32> = (0..n as u32)
        .filter(|&v| active[v as usize] && unknown_flag[v as usize])
        .collect();
    let prior = prior.map(|(p, idx, ..)| {
        let s = p.affine[0];
        // A node keeps its previous position only if it is still on the same side
        // of the known / unknown divide.
        let has: Vec<bool> = (0..n)
            .map(|v| idx[v] != NONE && p.unknown[idx[v] as usize] == unknown_flag[v])
            .collect();
        let pos: Vec<[f64; 2]> = idx
            .iter()
            .map(|&i| {
                if i == NONE {
                    return [0.0, 0.0];
                }
                let q = p.positions[i as usize];
                [
                    (q[0] as f64 - p.affine[1]) / s,
                    (q[1] as f64 - p.affine[2]) / s,
                ]
            })
            .collect();
        (pos, has, p.affine)
    });
    let (raw, runs, affine) = match &prior {
        Some((pos, has, affine)) => {
            let (mut raw, runs) = multilevel::warm(
                &g,
                &placed,
                pos,
                has,
                levels.last(),
                params.warm_iterations,
                &params.force,
                params.seed,
            );
            multilevel::place_unknown(&mut raw, &placed, &unknown_nodes, has, params.seed);
            concept::center_hubs(&mut raw, &hub, &concept_of);
            (raw, runs, *affine)
        }
        None => {
            let (mut raw, runs) = multilevel::cold(
                &g,
                &levels,
                &placed,
                params.iterations,
                params.refine_iterations,
                &params.force,
                params.seed,
            );
            multilevel::place_unknown(
                &mut raw,
                &placed,
                &unknown_nodes,
                &vec![false; n],
                params.seed,
            );
            concept::center_hubs(&mut raw, &hub, &concept_of);
            let affine = fit(&raw, &active);
            (raw, runs, affine)
        }
    };
    let mut positions: Vec<[f32; 2]> = raw
        .iter()
        .zip(&active)
        .map(|(p, &a)| {
            if !a {
                return [f32::NAN, f32::NAN];
            }
            let q = |v: f64, off: f64| (v * affine[0] + off).clamp(0.0, 1.0) as f32;
            [q(p[0], affine[1]), q(p[1], affine[2])]
        })
        .collect();
    let copy_merged = |p: &mut [[f32; 2]]| {
        for v in (0..n).filter(|&v| merged[v]) {
            p[v] = p[snap.canonical(v as u32) as usize];
        }
    };
    copy_merged(&mut positions);
    // What the next warm start continues from: the force layout, before the placement.
    let force_positions = positions.clone();
    // Declarations next to their concept.
    concept::place_members(&concept_of, &mut positions, &|v| snap.node_id(v));
    copy_merged(&mut positions);
    let secs_layout = t.elapsed().as_secs_f64();

    let strength = g.strength();
    for (l, (m, count)) in levels.iter().enumerate() {
        let (c, k) = centroids_and_central(&g, &strength, m, *count, &positions, &active);
        infos[l].centroids = c;
        infos[l].central = k;
    }
    let (edge_len_intra, edge_len_inter) = levels
        .last()
        .map_or((0.0, 0.0), |(m, _)| edge_lengths(&g, m, &positions));

    let t = Instant::now();
    let memberships: Vec<Vec<u32>> = levels.iter().map(|(m, _)| m.clone()).collect();
    let tiles = if opts.skip_tiles {
        TileSet {
            meta: TileMeta::default(),
            index: Vec::new(),
            blob: Bytes::Owned(Vec::new()),
        }
    } else {
        let counts: Vec<usize> = levels.iter().map(|l| l.1).collect();
        let centroids: Vec<Vec<[f32; 2]>> = infos.iter().map(|i| i.centroids.clone()).collect();
        let sizes: Vec<Vec<u32>> = infos.iter().map(|i| i.sizes.clone()).collect();
        let general = concept::general_set(
            snap,
            &active,
            &strength,
            params.tiles.general_nodes as usize,
        );
        let inp = tiles::Input {
            snap,
            pos: &positions,
            active: &active,
            score: &strength,
            levels: &memberships,
            counts: &counts,
            centroids: &centroids,
            sizes: &sizes,
            concept_of: &concept_of,
            general: &general,
        };
        tiles::build(&inp, &params.tiles)
    };
    let secs_tiles = t.elapsed().as_secs_f64();

    let version = {
        let mut h = blake3::Hasher::new();
        h.update(snap.content_hash_hex().as_bytes());
        h.update(ALGORITHM.as_bytes());
        h.update(format!("{params:?}").as_bytes());
        if let Some(w) = &warm {
            h.update(w.prior_version.as_bytes());
        }
        if let Some(ex) = &opts.exclude {
            h.update(&ex.iter().map(|&e| e as u8).collect::<Vec<_>>());
        }
        h.update(&[opts.skip_tiles as u8]);
        h.finalize().to_hex()[..16].to_string()
    };
    let stats = LayoutStats {
        nodes: n,
        active: active.iter().filter(|&&a| a).count(),
        merged: merged.iter().filter(|&&m| m).count(),
        unknown: (0..n).filter(|&v| active[v] && unknown_flag[v]).count(),
        layout_edges: g.num_edges(),
        secs_graph,
        secs_leiden,
        secs_layout,
        secs_tiles,
        secs_total: t0.elapsed().as_secs_f64(),
        runs,
        warm,
        prior_error: None,
        edge_len_intra,
        edge_len_inter,
    };
    Ok(Layout {
        version,
        snapshot_hash: snap.content_hash_hex(),
        positions,
        force_positions,
        levels: memberships,
        communities: infos,
        id_hash,
        affine,
        stats,
        tiles,
    })
}

/// Share of nodes with a seed label whose new community's most common seed label
/// is their own (1.0 = no community lost a member to another).
fn kept_share(seed: &[u32], idx: &[u32], m: &[u32], count: usize) -> f64 {
    let mut pairs: Vec<(u32, u32)> = (0..m.len())
        .filter(|&v| seed[v] != NONE && idx[v] != NONE)
        .map(|v| (m[v], seed[v]))
        .collect();
    if pairs.is_empty() {
        return 1.0;
    }
    pairs.par_sort_unstable();
    let mut best = vec![0usize; count];
    for run in pairs.chunk_by(|a, b| a == b) {
        let c = run[0].0 as usize;
        best[c] = best[c].max(run.len());
    }
    best.iter().sum::<usize>() as f64 / pairs.len() as f64
}

/// Final community ids: by active size (descending, ties by first member), then
/// one "unknown" community for nodes with no relation in the layers (and excluded
/// nodes); merged nodes take their survivor's community.
fn postprocess(
    snap: &Snapshot,
    active: &[bool],
    unknown_flag: &[bool],
    merged: &[bool],
    m: Vec<u32>,
) -> (Vec<u32>, usize, Option<u32>, Vec<u32>) {
    let n = m.len();
    let k = m.iter().copied().max().map_or(0, |x| x as usize + 1);
    let mut size = vec![0u32; k];
    let mut first = vec![u32::MAX; k];
    for v in 0..n {
        if active[v] && !unknown_flag[v] {
            size[m[v] as usize] += 1;
            first[m[v] as usize] = first[m[v] as usize].min(v as u32);
        }
    }
    let mut order: Vec<u32> = (0..k as u32).filter(|&c| size[c as usize] > 0).collect();
    order.sort_by_key(|&c| (std::cmp::Reverse(size[c as usize]), first[c as usize]));
    let mut map = vec![u32::MAX; k];
    for (i, &c) in order.iter().enumerate() {
        map[c as usize] = i as u32;
    }
    let mut sizes: Vec<u32> = order.iter().map(|&c| size[c as usize]).collect();
    let any_unknown = unknown_flag.iter().any(|&u| u);
    let unknown = any_unknown.then(|| {
        sizes.push((0..n).filter(|&v| active[v] && unknown_flag[v]).count() as u32);
        order.len() as u32
    });
    let mut out: Vec<u32> = (0..n)
        .map(|v| {
            if unknown_flag[v] {
                unknown.unwrap()
            } else {
                map[m[v] as usize]
            }
        })
        .collect();
    for v in 0..n {
        if merged[v] {
            out[v] = out[snap.canonical(v as u32) as usize];
        }
    }
    // A merged node whose survivor is itself merged or absent falls back to "unknown" or 0.
    for x in out.iter_mut() {
        if *x == u32::MAX {
            *x = unknown.unwrap_or(0);
        }
    }
    let count = sizes.len();
    (out, count, unknown, sizes)
}

/// Unit-square fit of the active nodes (uniform scale, centred, with a margin).
fn fit(raw: &[[f64; 2]], active: &[bool]) -> [f64; 3] {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (p, &a) in raw.iter().zip(active) {
        if a {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
    }
    if x0 > x1 {
        return [1.0, 0.5, 0.5];
    }
    let s = (1.0 - 2.0 * MARGIN) / (x1 - x0).max(y1 - y0).max(1e-12);
    [s, 0.5 - s * (x0 + x1) / 2.0, 0.5 - s * (y0 + y1) / 2.0]
}

type Central = (Vec<[f32; 2]>, Vec<[u32; 3]>);

fn centroids_and_central(
    g: &Graph,
    strength: &[f64],
    m: &[u32],
    count: usize,
    pos: &[[f32; 2]],
    active: &[bool],
) -> Central {
    let internal: Vec<f64> = (0..g.n())
        .into_par_iter()
        .map(|v| {
            let (ns, ws) = g.neighbors(v as u32);
            ns.iter()
                .zip(ws)
                .filter(|(u, _)| m[**u as usize] == m[v])
                .map(|(_, w)| w)
                .sum()
        })
        .collect();
    let (order, start) = group_by(m, count);
    (0..count)
        .into_par_iter()
        .map(|c| {
            let (mut s, mut k) = ([0.0f64; 2], 0usize);
            let mut top: Vec<u32> = Vec::with_capacity(4);
            let better = |a: u32, b: u32| {
                let (a, b) = (a as usize, b as usize);
                internal[a]
                    .total_cmp(&internal[b])
                    .then(strength[a].total_cmp(&strength[b]))
                    .then(b.cmp(&a))
                    .is_gt()
            };
            for &v in &order[start[c]..start[c + 1]] {
                if !active[v as usize] {
                    continue;
                }
                s[0] += pos[v as usize][0] as f64;
                s[1] += pos[v as usize][1] as f64;
                k += 1;
                let at = top.iter().position(|&t| better(v, t)).unwrap_or(top.len());
                if at < 3 {
                    top.insert(at, v);
                    top.truncate(3);
                }
            }
            let centroid = if k > 0 {
                [(s[0] / k as f64) as f32, (s[1] / k as f64) as f32]
            } else {
                [0.5, 0.5]
            };
            let mut central = [NONE; 3];
            central[..top.len()].copy_from_slice(&top);
            (centroid, central)
        })
        .unzip()
}

fn edge_lengths(g: &Graph, m: &[u32], pos: &[[f32; 2]]) -> (f64, f64) {
    let len = |v: usize, intra: bool| -> (f64, f64) {
        let (ns, _) = g.neighbors(v as u32);
        let (mut s, mut k) = (0.0, 0.0);
        for &u in ns {
            if (u as usize) > v && (m[u as usize] == m[v]) == intra {
                let (a, b) = (pos[v], pos[u as usize]);
                s += (((a[0] - b[0]) as f64).powi(2) + ((a[1] - b[1]) as f64).powi(2)).sqrt();
                k += 1.0;
            }
        }
        (s, k)
    };
    let (si, ki) = det_sum2(g.n(), |v| len(v, true));
    let (so, ko) = det_sum2(g.n(), |v| len(v, false));
    (
        if ki > 0.0 { si / ki } else { 0.0 },
        if ko > 0.0 { so / ko } else { 0.0 },
    )
}

pub(crate) fn tile(
    snap: &Snapshot,
    layout: &Layout,
    layer: Projection,
    z: u32,
    x: u32,
    y: u32,
    lang: &str,
) -> Result<TileResponse> {
    let meta = &layout.tiles.meta;
    if layout.tiles.meta.display_level.is_empty() {
        return Err(Error::NotFound(
            "this layout was built without tiles".into(),
        ));
    }
    if z > meta.zmax {
        return Err(Error::BadRequest(format!(
            "z={z} is beyond the max zoom {}",
            meta.zmax
        )));
    }
    if x as u64 >= 1u64 << z || y as u64 >= 1u64 << z {
        return Err(Error::BadRequest(format!(
            "tile ({x}, {y}) is outside zoom {z}"
        )));
    }
    let size = 1.0 / (1u64 << z) as f32;
    let mut resp = TileResponse {
        layer,
        layout_version: layout.version.clone(),
        z,
        x,
        y,
        max_zoom: meta.zmax,
        bbox: [
            x as f32 * size,
            y as f32 * size,
            (x + 1) as f32 * size,
            (y + 1) as f32 * size,
        ],
        nodes: Vec::new(),
        external: Vec::new(),
        edges: Vec::new(),
        aggregates: Vec::new(),
        hidden_edges: BTreeMap::new(),
    };
    let Some(blob) = layout.tiles.get(z, x, y) else {
        return Ok(resp);
    };
    let d = tiles::decode(blob, z, x, y)
        .map_err(|e| Error::Integrity(format!("tile {z}/{x}/{y}: {e}")))?;
    let dl = meta.display_level[z as usize].map(|l| l as usize);
    let entity_level = dl.unwrap_or(layout.levels.len() - 1);
    let in_layer = |rel: u8| RelationType::from_code(rel).filter(|r| r.spec().projection == layer);
    let entity = |v: u32, rank: Option<u32>, px: f32, py: f32| TileNode {
        id: snap.node_id(v).to_string(),
        kind: if snap.concept_index(v).is_some() {
            TileNodeKind::Concept
        } else {
            TileNodeKind::Entity
        },
        label: snap.label(v, lang).to_string(),
        x: px,
        y: py,
        size: if snap.concept_index(v).is_some() {
            snap.concept_members(v).len() as u32
        } else {
            1
        },
        community: layout.community_id(entity_level, layout.levels[entity_level][v as usize]),
        entity_type: Some(snap.node_type(v)),
        priority: rank,
        central: None,
    };
    let cluster = |c: u32, px: f32, py: f32, hidden: u32| {
        let l = dl.expect("clusters only below zmax");
        let id = layout.community_id(l, c);
        let central = layout.communities[l].central.get(c as usize).map(|m| {
            m.iter()
                .filter(|&&v| v != NONE)
                .map(|&v| snap.node_id(v).to_string())
                .collect()
        });
        TileNode {
            label: layout.region_label(snap, l, c, lang),
            community: id.clone(),
            id,
            kind: TileNodeKind::Cluster,
            x: px,
            y: py,
            size: hidden,
            entity_type: None,
            priority: None,
            central,
        }
    };
    let layer_rels = |rels: &[(u8, u32)]| {
        let mut hr = BTreeMap::new();
        for &(rel, count) in rels {
            if let Some(r) = in_layer(rel) {
                *hr.entry(r).or_insert(0) += count;
            }
        }
        hr
    };
    for (&(v, rank, px, py), (hidden, rels)) in d.entities.iter().zip(&d.entity_hidden) {
        let node = entity(v, Some(rank), px, py);
        if node.kind == TileNodeKind::Concept {
            resp.aggregates.push(Aggregate {
                cluster_id: node.id.clone(),
                members: node.size,
                hidden_members: *hidden,
                hidden_relations: layer_rels(rels),
            });
        }
        resp.nodes.push(node);
    }
    for (c, px, py, members, hidden, rels) in &d.clusters {
        let node = cluster(*c, *px, *py, *hidden);
        resp.aggregates.push(Aggregate {
            cluster_id: node.id.clone(),
            members: *members,
            hidden_members: *hidden,
            hidden_relations: layer_rels(rels),
        });
        resp.nodes.push(node);
    }
    let local = resp.nodes.len();
    let mut ext_used: Vec<Option<usize>> = vec![None; d.external.len()];
    for &(a, b, rel, state, count) in &d.edges {
        let Some(relation) = in_layer(rel) else {
            continue;
        };
        let mut id_of = |i: u32| -> String {
            let i = i as usize;
            if i < local {
                return resp.nodes[i].id.clone();
            }
            let e = i - local;
            if ext_used[e].is_none() {
                let (r, is_c, px, py) = d.external[e];
                let node = if is_c {
                    cluster(r, px, py, 0)
                } else {
                    entity(r, None, px, py)
                };
                ext_used[e] = Some(resp.external.len());
                resp.external.push(node);
            }
            resp.external[ext_used[e].unwrap()].id.clone()
        };
        let (source, target) = (id_of(a), id_of(b));
        let evidence_state = EvidenceState::from_code(state)
            .ok_or_else(|| Error::Integrity(format!("tile {z}/{x}/{y}: bad state code {state}")))?;
        resp.edges.push(TileEdge {
            source,
            target,
            relation,
            evidence_state,
            count,
        });
    }
    for &(rel, count) in &d.hidden_edges {
        if let Some(r) = in_layer(rel) {
            *resp.hidden_edges.entry(r).or_insert(0) += count;
        }
    }
    Ok(resp)
}
