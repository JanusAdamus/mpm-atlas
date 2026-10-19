//! End-to-end checks: Leiden quality on known graphs, and the full pipeline on the
//! committed fixture registry (core/atlas-core/tests/fixtures/mini_atlas.db).

use atlas_core::{CompileOptions, Dir, Projection, RelationType, Snapshot, compile};
use atlas_layout::graph::Graph;
use atlas_layout::leiden::{self, LeidenParams, Quality};
use atlas_layout::{
    ComputeOptions, Layout, LayoutParams, Prior, TileBudget, TileNodeKind, compute, compute_with,
    tile,
};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::PathBuf;

const KARATE: [(u32, u32); 78] = [
    (0, 1),
    (0, 2),
    (0, 3),
    (0, 4),
    (0, 5),
    (0, 6),
    (0, 7),
    (0, 8),
    (0, 10),
    (0, 11),
    (0, 12),
    (0, 13),
    (0, 17),
    (0, 19),
    (0, 21),
    (0, 31),
    (1, 2),
    (1, 3),
    (1, 7),
    (1, 13),
    (1, 17),
    (1, 19),
    (1, 21),
    (1, 30),
    (2, 3),
    (2, 7),
    (2, 8),
    (2, 9),
    (2, 13),
    (2, 27),
    (2, 28),
    (2, 32),
    (3, 7),
    (3, 12),
    (3, 13),
    (4, 6),
    (4, 10),
    (5, 6),
    (5, 10),
    (5, 16),
    (6, 16),
    (8, 30),
    (8, 32),
    (8, 33),
    (9, 33),
    (13, 33),
    (14, 32),
    (14, 33),
    (15, 32),
    (15, 33),
    (18, 32),
    (18, 33),
    (19, 33),
    (20, 32),
    (20, 33),
    (22, 32),
    (22, 33),
    (23, 25),
    (23, 27),
    (23, 29),
    (23, 32),
    (23, 33),
    (24, 25),
    (24, 27),
    (24, 31),
    (25, 31),
    (26, 29),
    (26, 33),
    (27, 33),
    (28, 31),
    (28, 33),
    (29, 32),
    (29, 33),
    (30, 32),
    (30, 33),
    (31, 32),
    (31, 33),
    (32, 33),
];

fn karate() -> Graph {
    let e: Vec<(u32, u32, f64)> = KARATE.iter().map(|&(a, b)| (a, b, 1.0)).collect();
    Graph::from_edges(34, &e)
}

/// Every community induces a connected subgraph (Leiden's guarantee).
fn assert_connected(g: &Graph, m: &[u32], count: usize) {
    let mut seen = vec![false; g.n()];
    let mut comps = 0;
    for s in 0..g.n() {
        if seen[s] {
            continue;
        }
        comps += 1;
        seen[s] = true;
        let mut q = VecDeque::from([s as u32]);
        while let Some(v) = q.pop_front() {
            for &u in g.neighbors(v).0 {
                if !seen[u as usize] && m[u as usize] == m[v as usize] {
                    seen[u as usize] = true;
                    q.push_back(u);
                }
            }
        }
    }
    assert_eq!(comps, count, "a community is disconnected");
}

#[test]
fn karate_modularity_near_optimum() {
    let g = karate();
    let p = leiden::leiden(&g, &LeidenParams::default(), None);
    let s = leiden::scores(&g, &p.membership, p.count, Quality::Modularity, 1.0);
    // Known optimum 0.4198 (Brandes et al. 2008).
    assert!(
        s.modularity > 0.41 && s.modularity < 0.4199,
        "Q = {}",
        s.modularity
    );
    assert_connected(&g, &p.membership, p.count);
}

fn planted(groups: u32, size: u32, k_in: u32, k_out: u32, seed: u64) -> (Graph, Vec<u32>) {
    let mut x = seed;
    let mut next = move || {
        x = x
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (x >> 33) as u32
    };
    let n = groups * size;
    let mut e = Vec::new();
    for v in 0..n {
        let g = v / size;
        for _ in 0..k_in {
            e.push((v, g * size + next() % size, 1.0));
        }
        for _ in 0..k_out {
            e.push((v, next() % n, 1.0));
        }
    }
    (
        Graph::from_edges(n as usize, &e),
        (0..n).map(|v| v / size).collect(),
    )
}

#[test]
fn planted_partition_recovered_and_hierarchy_nested() {
    let (g, truth) = planted(20, 100, 8, 1, 7);
    let parts = leiden::hierarchy(&g, &[0.05, 1.0, 4.0], &LeidenParams::default());
    let mid = &parts[1].partition;
    // purity: the largest share of each planted group inside one community
    let mut agree = 0;
    for grp in 0..20u32 {
        let mut c: HashMap<u32, usize> = HashMap::new();
        for v in (0..2000).filter(|&v| truth[v] == grp) {
            *c.entry(mid.membership[v]).or_default() += 1;
        }
        agree += c.values().max().unwrap();
    }
    assert!(
        agree as f64 / 2000.0 > 0.95,
        "purity {}",
        agree as f64 / 2000.0
    );
    // nesting: every fine community sits inside one coarser community
    for w in parts.windows(2) {
        let (coarse, fine) = (&w[0].partition, &w[1].partition);
        let mut parent: HashMap<u32, u32> = HashMap::new();
        for v in 0..g.n() {
            let p = parent
                .entry(fine.membership[v])
                .or_insert(coarse.membership[v]);
            assert_eq!(*p, coarse.membership[v], "level not nested");
        }
        assert!(coarse.count <= fine.count);
    }
    for l in &parts {
        assert_connected(&g, &l.partition.membership, l.partition.count);
    }
}

#[test]
fn max_share_splits_a_collapsed_coarse_level() {
    // At γ = 0.01 modularity folds the whole connected graph into one community.
    let (g, _) = planted(20, 100, 8, 1, 7);
    let res = [0.01, 4.0];
    let plain = leiden::hierarchy(&g, &res, &LeidenParams::default());
    assert_eq!(
        plain[0].partition.count, 1,
        "the test needs a collapsed coarse level"
    );
    let capped = leiden::hierarchy_seeded(&g, &res, &LeidenParams::default(), &[], &[], 0.25);
    let (coarse, fine) = (&capped[0].partition, &capped[1].partition);
    let mut size = vec![0usize; coarse.count];
    coarse
        .membership
        .iter()
        .for_each(|&c| size[c as usize] += 1);
    let largest = *size.iter().max().unwrap() as f64 / g.n() as f64;
    assert!(largest <= 0.25, "largest coarse community holds {largest}");
    assert!(
        capped[0].resolution > 0.01 && capped[0].resolution <= 4.0,
        "used γ {}",
        capped[0].resolution
    );
    assert_eq!(
        capped[1].resolution, 4.0,
        "the finest level is never changed"
    );
    assert_eq!(fine.membership, plain[1].partition.membership);
    let mut parent: HashMap<u32, u32> = HashMap::new();
    for v in 0..g.n() {
        assert_eq!(
            *parent
                .entry(fine.membership[v])
                .or_insert(coarse.membership[v]),
            coarse.membership[v],
            "not nested"
        );
    }
    assert_connected(&g, &coarse.membership, coarse.count);
}

fn fixture_snapshot(name: &str) -> (Snapshot, PathBuf) {
    let db = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../atlas-core/tests/fixtures/mini_atlas.db");
    let dir = std::env::temp_dir().join(format!("atlas-layout-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    compile(&CompileOptions::new(db, dir.join("snapshot"))).expect("compile fixture");
    (Snapshot::open(dir.join("snapshot")).unwrap(), dir)
}

/// Tiny budgets so the 19-node fixture spans several zooms, clusters and hidden nodes.
fn small_params() -> LayoutParams {
    LayoutParams {
        resolutions: vec![0.5, 2.0],
        tiles: TileBudget {
            entities: 3,
            clusters: 1,
            edges: 1000,
            full: 4,
            max_zoom: 8,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn fixture_layout_is_deterministic_and_honest() {
    let (s, _) = fixture_snapshot("det");
    let p = small_params();
    let a = compute(&s, &p).unwrap();
    let b = compute(&s, &p).unwrap();
    assert_eq!(a.version, b.version);
    assert_eq!(a.positions, b.positions);
    assert_eq!(
        a.tiles.keys().collect::<Vec<_>>(),
        b.tiles.keys().collect::<Vec<_>>()
    );
    for (z, x, y) in a.tiles.keys() {
        assert_eq!(a.tiles.get(z, x, y), b.tiles.get(z, x, y));
    }
    let lonely = s.resolve("c:lonely").unwrap();
    let dup = s.find_node("s:pythagoras-dup").unwrap();
    let pyth = s.resolve("s:pythagoras").unwrap();
    for (l, info) in a.communities.iter().enumerate() {
        let u = info.unknown.expect("c:lonely has no relations");
        assert_eq!(a.levels[l][lonely as usize], u, "unknown, not isolated");
        assert_eq!(a.community_id(l, u), format!("L{l}:unknown"));
        assert_eq!(a.region_label(&s, l, u, "es"), "Relaciones desconocidas");
        assert_eq!(
            a.levels[l][dup as usize], a.levels[l][pyth as usize],
            "merged node follows its survivor"
        );
        assert!(info.modularity.is_finite());
    }
    assert_eq!(a.positions[dup as usize], a.positions[pyth as usize]);
    // Unknown nodes sit apart, right of every node with relations (not among them).
    let u = a.communities.last().unwrap().unknown;
    let last = a.levels.len() - 1;
    let right_of_map = (0..a.positions.len())
        .filter(|&v| a.positions[v][0].is_finite() && Some(a.levels[last][v]) != u)
        .map(|v| a.positions[v][0])
        .fold(f32::MIN, f32::max);
    assert!(
        a.positions[lonely as usize][0] > right_of_map,
        "unknown node placed among the map"
    );
    assert!(
        a.positions
            .iter()
            .all(|p| (0.0..=1.0).contains(&p[0]) && (0.0..=1.0).contains(&p[1]))
    );
    assert!(a.tiles.meta.zmax >= 1, "small budgets need several zooms");
    // The merged duplicate never shows on a tile.
    for (z, x, y) in a.tiles.keys() {
        let t = tile(&s, &a, Projection::Structure, z, x, y, "es").unwrap();
        assert!(t.nodes.iter().all(|n| n.id != "s:pythagoras-dup"));
    }
}

/// Per zoom, the union of tile edges over all layers equals the real edges mapped
/// to their representatives: nothing invented, nothing lost, one relation and one
/// state per aggregated edge.  Internal edges add up to the clusters' hidden relations.
#[test]
fn aggregated_edges_are_exactly_the_real_edges() {
    let (s, _) = fixture_snapshot("agg");
    let l = compute(&s, &small_params()).unwrap();
    let zmax = l.tiles.meta.zmax;
    let out = s.csr(Dir::Out);
    for z in 0..=zmax {
        let mut visible: HashSet<String> = HashSet::new();
        let mut got: BTreeMap<(String, String, RelationType, String), u32> = BTreeMap::new();
        let mut internal: BTreeMap<(String, RelationType), u32> = BTreeMap::new();
        let mut members_shown = 0;
        for (tz, x, y) in l.tiles.keys().filter(|k| k.0 == z) {
            for &layer in Projection::ALL.iter() {
                let t = tile(&s, &l, layer, tz, x, y, "en").unwrap();
                assert!(t.hidden_edges.is_empty(), "budget is large enough here");
                for n in t.nodes.iter().filter(|n| n.kind == TileNodeKind::Entity) {
                    visible.insert(n.id.clone());
                }
                for e in &t.edges {
                    assert_eq!(e.relation.spec().projection, layer);
                    let k = (
                        e.source.clone(),
                        e.target.clone(),
                        e.relation,
                        e.evidence_state.to_string(),
                    );
                    let prev = got.insert(k, e.count);
                    assert!(
                        prev.is_none() || prev == Some(e.count),
                        "same edge, two counts"
                    );
                }
                for a in &t.aggregates {
                    for (r, c) in &a.hidden_relations {
                        internal.insert((a.cluster_id.clone(), *r), *c);
                    }
                    if layer == Projection::Structure {
                        members_shown += a.hidden_members;
                    }
                }
            }
        }
        let dl = l.tiles.meta.display_level[z as usize].map(|d| d as usize);
        let rep = |v: u32| -> String {
            let id = s.node_id(v).to_string();
            if visible.contains(&id) {
                id
            } else {
                let d = dl.expect("hidden nodes only below zmax");
                l.community_id(d, l.levels[d][v as usize])
            }
        };
        let mut want: BTreeMap<(String, String, RelationType, String), u32> = BTreeMap::new();
        let mut want_internal: BTreeMap<(String, RelationType), u32> = BTreeMap::new();
        for u in 0..s.num_nodes() {
            if s.is_merged(u) {
                continue;
            }
            for e in out.edges(u).iter() {
                let (a, b) = (rep(u), rep(e.nbr));
                if a == b {
                    *want_internal.entry((a, e.relation)).or_default() += 1;
                } else {
                    *want
                        .entry((a, b, e.relation, e.state.to_string()))
                        .or_default() += 1;
                }
            }
        }
        assert_eq!(got, want, "zoom {z}");
        assert_eq!(internal, want_internal, "zoom {z} internal");
        let active = (0..s.num_nodes()).filter(|&v| !s.is_merged(v)).count();
        assert_eq!(
            visible.len() + members_shown as usize,
            active,
            "every node is either visible or in a cluster at zoom {z}"
        );
        if z == zmax {
            assert_eq!(visible.len(), active, "everything is visible at zmax");
        }
    }
}

#[test]
fn entity_budget_respected_below_zmax() {
    let (s, _) = fixture_snapshot("budget");
    let p = small_params();
    let l = compute(&s, &p).unwrap();
    for (z, x, y) in l.tiles.keys() {
        let t = tile(&s, &l, Projection::Proof, z, x, y, "es").unwrap();
        let ents = t
            .nodes
            .iter()
            .filter(|n| n.kind == TileNodeKind::Entity)
            .count();
        if z < l.tiles.meta.zmax {
            assert!(
                ents <= p.tiles.entities as usize,
                "tile {z}/{x}/{y} has {ents} entities"
            );
        }
        let ranks: Vec<u32> = t.nodes.iter().filter_map(|n| n.priority).collect();
        assert!(
            ranks.windows(2).all(|w| w[0] < w[1]),
            "entities sorted by label priority"
        );
    }
    // Once visible, always visible.
    let mut first: HashMap<String, u32> = HashMap::new();
    for (z, x, y) in l.tiles.keys() {
        for n in tile(&s, &l, Projection::Proof, z, x, y, "es")
            .unwrap()
            .nodes
        {
            if n.kind == TileNodeKind::Entity {
                first.entry(n.id).or_insert(z);
            }
        }
    }
    for z in 0..=l.tiles.meta.zmax {
        let shown: HashSet<String> = l
            .tiles
            .keys()
            .filter(|k| k.0 == z)
            .flat_map(|(z, x, y)| {
                tile(&s, &l, Projection::Proof, z, x, y, "es")
                    .unwrap()
                    .nodes
            })
            .map(|n| n.id)
            .collect();
        for (id, &fz) in &first {
            if fz <= z {
                assert!(shown.contains(id), "{id} visible at {fz} but not at {z}");
            }
        }
    }
}

#[test]
fn write_open_roundtrip_and_snapshot_check() {
    let (s, dir) = fixture_snapshot("io");
    let l = compute(&s, &small_params()).unwrap();
    let out = dir.join("layout");
    let r = l.write(&out).unwrap();
    assert!(r.layout_bytes > 0 && r.tiles_bytes > 0);
    let o = Layout::open(&s, &out).unwrap();
    assert_eq!(o.version, l.version);
    assert_eq!(o.positions, l.positions);
    assert_eq!(o.levels, l.levels);
    assert_eq!(o.communities[0].central, l.communities[0].central);
    for (z, x, y) in l.tiles.keys() {
        assert_eq!(o.tiles.get(z, x, y), l.tiles.get(z, x, y));
    }
    let json =
        serde_json::to_value(tile(&s, &o, Projection::Structure, 0, 0, 0, "es").unwrap()).unwrap();
    for k in [
        "layer",
        "layout_version",
        "bbox",
        "nodes",
        "edges",
        "aggregates",
        "max_zoom",
    ] {
        assert!(json.get(k).is_some(), "missing {k}");
    }
    assert_eq!(json["layer"], "structure");
    // Compact coordinates (round 3): at most the stored precision, 5 decimals at z0
    // (before: the f32 noise, ~9 digits per coordinate).
    for (z, x, y) in l.tiles.keys().filter(|k| k.0 <= 1) {
        let t = serde_json::to_string(&tile(&s, &o, Projection::Structure, z, x, y, "es").unwrap()).unwrap();
        let max = if z == 0 { 5 } else { 6 };
        for key in ["\"x\":", "\"y\":"] {
            for part in t.split(key).skip(1) {
                let num: String = part.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                let decimals = num.split('.').nth(1).map_or(0, str::len);
                assert!(decimals <= max, "z{z}: coordinate {num} has {decimals} decimals");
            }
        }
    }
    assert!(
        tile(
            &s,
            &o,
            Projection::Structure,
            o.tiles.meta.zmax + 1,
            0,
            0,
            "es"
        )
        .is_err()
    );
    assert!(tile(&s, &o, Projection::Structure, 1, 2, 0, "es").is_err());
    // A tampered payload is caught.
    let lay = r.layout_file.clone();
    let mut bytes = std::fs::read(&lay).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    std::fs::write(&lay, bytes).unwrap();
    assert!(Layout::open(&s, &out).is_err());
}

#[test]
fn warm_start_keeps_existing_nodes_in_place() {
    let (s, _) = fixture_snapshot("warm");
    let p = LayoutParams {
        resolutions: vec![1.0],
        ..Default::default()
    };
    let n = s.num_nodes() as usize;
    let added = s.resolve("s:law-of-cosines").unwrap() as usize;
    let mut exclude = vec![false; n];
    exclude[added] = true;
    let old = compute_with(
        &s,
        &p,
        &ComputeOptions {
            exclude: Some(exclude),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        old.positions[added][0].is_nan(),
        "excluded node has no position"
    );
    let prior = Prior::from_layout(&old);
    let new = compute_with(
        &s,
        &p,
        &ComputeOptions {
            prior: Some(prior),
            ..Default::default()
        },
    )
    .unwrap();
    let w = new.stats.warm.as_ref().expect("warm start used");
    assert_eq!(w.new, 1);
    assert_ne!(new.version, old.version);
    let moved: Vec<f32> = (0..n)
        .filter(|&v| v != added && !s.is_merged(v as u32))
        .map(|v| {
            ((new.positions[v][0] - old.positions[v][0]).powi(2)
                + (new.positions[v][1] - old.positions[v][1]).powi(2))
            .sqrt()
        })
        .collect();
    let mean = moved.iter().sum::<f32>() / moved.len() as f32;
    assert!(mean < 0.05, "existing nodes moved {mean} on average");
    assert!(new.positions[added][0].is_finite());
}

#[test]
fn bad_params_are_rejected() {
    let (s, _) = fixture_snapshot("bad");
    for p in [
        LayoutParams {
            resolutions: vec![],
            ..Default::default()
        },
        LayoutParams {
            resolutions: vec![f64::NAN],
            ..Default::default()
        },
        LayoutParams {
            layers: vec![],
            ..Default::default()
        },
    ] {
        assert_eq!(compute(&s, &p).err().map(|e| e.code()), Some("bad_request"));
    }
}

/// Share of nodes whose community in `b` is the one most of their `a`-community went to.
fn agreement(a: &[u32], b: &[u32]) -> f64 {
    let mut c: HashMap<(u32, u32), usize> = HashMap::new();
    for (x, y) in a.iter().zip(b) {
        *c.entry((*x, *y)).or_default() += 1;
    }
    let mut best: HashMap<u32, usize> = HashMap::new();
    for ((x, _), k) in c {
        let e = best.entry(x).or_default();
        *e = (*e).max(k);
    }
    best.values().sum::<usize>() as f64 / a.len() as f64
}

#[test]
fn seeded_hierarchy_keeps_previous_communities() {
    // Weak structure (k_out > k_in / 2): different seeds give different partitions.
    let (g, _) = planted(40, 300, 4, 3, 11);
    assert!(g.n() >= 8192, "exercise the batched local moving too");
    let res = [0.5, 2.0];
    let a = leiden::hierarchy(
        &g,
        &res,
        &LeidenParams {
            seed: 1,
            ..Default::default()
        },
    );
    let other = LeidenParams {
        seed: 99,
        ..Default::default()
    };
    let cold = leiden::hierarchy(&g, &res, &other);
    let seeds: Vec<Option<Vec<u32>>> = a
        .iter()
        .map(|l| Some(l.partition.membership.clone()))
        .collect();
    let warm = leiden::hierarchy_seeded(&g, &res, &other, &seeds, &[], 1.0);
    for l in 0..res.len() {
        let (pa, pc, pw) = (
            &a[l].partition.membership,
            &cold[l].partition.membership,
            &warm[l].partition.membership,
        );
        let (kept_cold, kept_warm) = (agreement(pa, pc), agreement(pa, pw));
        assert!(kept_warm > 0.95, "level {l}: seeded run kept {kept_warm}");
        assert!(
            kept_warm >= kept_cold,
            "level {l}: seeded {kept_warm} < cold {kept_cold}"
        );
        assert_connected(&g, pw, warm[l].partition.count);
    }
}
