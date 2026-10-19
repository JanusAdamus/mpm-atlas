//! Precomputed z/x/y tiles over the unit square (quadtree; tile (z, x, y) covers
//! [x, x+1) × [y, y+1) / 2^z, y downwards).
//!
//! What a tile shows (GraphMaps-style levels of detail, R36):
//! - Entities: each node gets a minimum zoom by label priority (weighted degree,
//!   ties by index).  Taking nodes in priority order, a node becomes visible at the
//!   smallest zoom from which every tile containing it, at that zoom and all deeper
//!   ones, still has room under `entities` visible nodes.  At `zmax` every node is
//!   visible.  So no tile below `zmax` holds more than `entities` entities, and a
//!   node stays visible once it appears.
//! - Clusters: nodes not yet visible are shown through their community at the
//!   zoom's display level (the finest Leiden level with at most `clusters · 4^z`
//!   communities), drawn at the centroid of all its members (stable across zooms).
//! - Edges: every current claim edge is mapped to (representative of subject,
//!   representative of object, relation, evidence state) and counted.  An
//!   aggregated edge therefore exists only if at least one real edge maps to it and
//!   never mixes relations or states.  Edges inside one cluster are counted in that
//!   cluster's `hidden_relations`.  A tile carries the edges touching its items,
//!   largest counts first, up to `edges`; the rest are counted per relation in the
//!   tile's `hidden_edges`, never dropped silently.
//!
//! Concept layer (when the snapshot has concepts; `concept.rs`):
//! - Zooms `0..=general_zoom` are the general layer.  They show only the general
//!   set (concepts, then areas; at most `general_nodes`) and no clusters.  A general
//!   node takes its zoom by priority like an entity, under its own per-tile cap
//!   `general_tile_nodes` (round 3: with every node at zoom 0 the z0 tile weighed
//!   523 KB, and the z3 tiles under it held up to 1,300 nodes).  Until it fits it is
//!   not represented and its edges are counted, as below; at `zmax` all are drawn.
//! - A declaration is drawn only from `detail_zoom` on (infrastructure too); other
//!   entities from `general_zoom + 1`.  Above that, the budget rule applies.
//! - A hidden declaration is represented by its primary concept when the concept
//!   is visible: its edges are counted on the concept, edges between members of one
//!   concept go to that concept's hidden relations.
//! - `member_of_concept` claims are never drawn as edges, at any zoom: they are the
//!   grouping itself, shown as placement (a declaration sits in its primary
//!   concept's disc) and member counts; `/api/concept` lists every one with its
//!   evidence.  Drawn, they were 57 % of the deep-zoom edges, mostly long lines to
//!   secondary concepts.
//! - In the general layer, a hidden node with no visible concept (sources, people,
//!   infrastructure...) is not represented: an edge with one such endpoint is
//!   counted in the other endpoint's tile `hidden_edges`; one with two is counted
//!   in the zoom's `edges_unplaced`.  Nothing is dropped silently.
//!
//! Binary tile (all integers LEB128 varints unless noted):
//!   ne nc nx nedges nhidden
//!   ne × entity   : node, rank, qx u16, qy u16, hidden_members, nrel, nrel × (rel u8, count)
//!                   (sorted by rank = label priority; hidden members and internal
//!                   relations are those a visible concept stands for)
//!   nc × cluster  : community, qx u16, qy u16, members, hidden_members, nrel, nrel × (rel u8, count)
//!   nx × external : ref (node<<1 | is_cluster), gx u32, gy u32    (endpoints outside the tile)
//!   nedges × edge : a, b (local: entities, then clusters, then externals), rel u8, state u8, count
//!   nhidden × (rel u8, count)
//! qx/qy are fixed point inside the tile; gx/gy fixed point in the unit square.

use ahash::AHashMap;
use atlas_core::{Dir, Snapshot};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::util::CHUNK;

pub(crate) const CLUSTER_BIT: u32 = 1 << 31;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileBudget {
    /// Max visible entities per tile below `zmax`.
    pub entities: u32,
    /// Display level: finest with at most `clusters · 4^z` communities.
    pub clusters: u32,
    /// Max edges per tile and projection (the rest are counted in `hidden_edges`).
    pub edges: u32,
    /// `zmax` is the first zoom whose fullest tile holds at most this many nodes...
    pub full: u32,
    /// ...or this zoom, whichever comes first.
    pub max_zoom: u32,
    /// Zooms `0..=general_zoom` show only concepts and areas (with a concept layer).
    #[serde(default = "default_general_zoom")]
    pub general_zoom: u32,
    /// Most nodes in the general layer.
    #[serde(default = "default_general_nodes")]
    pub general_nodes: u32,
    /// First zoom at which declarations (and infrastructure) can be drawn.
    #[serde(default = "default_detail_zoom")]
    pub detail_zoom: u32,
    /// Max edges per tile and projection in the general layer (fewer lines over the overview; the
    /// rest are counted in `hidden_edges` like any edge over budget).
    #[serde(default = "default_general_edges")]
    pub general_edges: u32,
    /// Max visible nodes of the general set per tile (at every zoom below `zmax`).
    #[serde(default = "default_general_tile_nodes")]
    pub general_tile_nodes: u32,
}

fn default_general_tile_nodes() -> u32 {
    GENERAL_TILE_NODES
}

/// Default of `general_tile_nodes`.  Round 3 sweep on the real snapshot
/// (`examples/tile_budget.rs`, AtlasBench mix): 600 nodes and 512 lines gave a z0 of
/// 190 KB and a p95 tile of 231 KB of JSON; 400 and 384 give 131 KB and 203 KB.
const GENERAL_TILE_NODES: u32 = 400;

fn default_general_edges() -> u32 {
    384
}

fn default_general_zoom() -> u32 {
    2
}
fn default_general_nodes() -> u32 {
    6000
}
fn default_detail_zoom() -> u32 {
    4
}

impl Default for TileBudget {
    fn default() -> Self {
        TileBudget {
            entities: 256,
            clusters: 64,
            // Round 3: 2,048 made the z3 tiles 550-720 KB of JSON (AtlasBench tile p95
            // 8.78 ms); 384 lines per layer and tile is still more than a screen reads.
            edges: 384,
            full: 1024,
            max_zoom: 16,
            general_zoom: default_general_zoom(),
            general_nodes: default_general_nodes(),
            detail_zoom: default_detail_zoom(),
            general_edges: default_general_edges(),
            general_tile_nodes: default_general_tile_nodes(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ZoomStats {
    pub z: u32,
    pub display_level: Option<u32>,
    pub tiles: usize,
    pub visible_entities: usize,
    pub clusters: usize,
    pub aggregated_edges: usize,
    pub bytes_total: u64,
    pub bytes_p50: u32,
    pub bytes_p95: u32,
    pub bytes_max: u32,
    pub max_entities: u32,
    pub max_edges: u32,
    pub tiles_with_hidden_edges: usize,
    /// Visible concepts, and hidden nodes each of them stands for.
    #[serde(default)]
    pub concepts: usize,
    #[serde(default)]
    pub represented_by_concept: usize,
    /// General layer: hidden nodes with no visible concept (not represented), and
    /// edges between two of them (counted here, in no tile).
    #[serde(default)]
    pub unplaced: usize,
    #[serde(default)]
    pub edges_unplaced: u64,
    pub secs: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TileMeta {
    pub zmax: u32,
    /// Display level per zoom (`None` at `zmax`, where every node is visible).
    pub display_level: Vec<Option<u32>>,
    pub budget: TileBudget,
    pub count: usize,
    pub zooms: Vec<ZoomStats>,
    /// Last zoom of the general layer, if the snapshot has a concept layer.
    #[serde(default)]
    pub general_zoom: Option<u32>,
    /// Nodes in the general layer.
    #[serde(default)]
    pub general_nodes: usize,
}

/// Tile bytes, owned after a build or memory-mapped after `Layout::open`.
pub(crate) enum Bytes {
    Owned(Vec<u8>),
    Mapped(memmap2::Mmap, usize),
}

impl Bytes {
    pub(crate) fn as_slice(&self) -> &[u8] {
        match self {
            Bytes::Owned(v) => v,
            Bytes::Mapped(m, off) => &m[*off..],
        }
    }
}

pub struct TileSet {
    pub meta: TileMeta,
    /// (key, offset, len) sorted by key; key = z << 48 | x << 24 | y.
    pub(crate) index: Vec<[u64; 3]>,
    pub(crate) blob: Bytes,
}

pub(crate) fn key(z: u32, x: u32, y: u32) -> u64 {
    (z as u64) << 48 | (x as u64) << 24 | y as u64
}

impl TileSet {
    pub fn get(&self, z: u32, x: u32, y: u32) -> Option<&[u8]> {
        let k = key(z, x, y);
        let i = self.index.binary_search_by_key(&k, |e| e[0]).ok()?;
        let [_, off, len] = self.index[i];
        Some(&self.blob.as_slice()[off as usize..(off + len) as usize])
    }

    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Every stored tile as (z, x, y), in key order.
    pub fn keys(&self) -> impl Iterator<Item = (u32, u32, u32)> + '_ {
        self.index.iter().map(|e| {
            (
                (e[0] >> 48) as u32,
                ((e[0] >> 24) & 0xFF_FFFF) as u32,
                (e[0] & 0xFF_FFFF) as u32,
            )
        })
    }

    /// Total bytes of tile data (index excluded).
    pub fn data_bytes(&self) -> u64 {
        self.index.iter().map(|e| e[2]).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

#[inline]
fn cell(p: [f32; 2], z: u32) -> (u32, u32) {
    let s = (1u64 << z) as f64;
    let max = (1u64 << z) as u32 - 1;
    let c = |v: f32| ((v as f64 * s).floor().max(0.0) as u64).min(max as u64) as u32;
    (c(p[0]), c(p[1]))
}

/// Everything the builder needs, all indexed by snapshot node or community.
pub(crate) struct Input<'a> {
    pub snap: &'a Snapshot,
    pub pos: &'a [[f32; 2]],
    pub active: &'a [bool],
    pub score: &'a [f64],
    pub levels: &'a [Vec<u32>],
    pub counts: &'a [usize],
    pub centroids: &'a [Vec<[f32; 2]>],
    pub sizes: &'a [Vec<u32>],
    /// Primary concept of each declaration (`NONE` otherwise).
    pub concept_of: &'a [u32],
    /// Nodes of the general layer (all false without a concept layer).
    pub general: &'a [bool],
}

impl Input<'_> {
    fn has_concepts(&self) -> bool {
        self.general.iter().any(|&g| g)
    }
}

/// Not represented at this zoom (general layer only).
const UNPLACED: u32 = u32::MAX;

pub(crate) fn build(inp: &Input, budget: &TileBudget) -> TileSet {
    let n = inp.pos.len();
    let mut order: Vec<u32> = (0..n as u32).filter(|&v| inp.active[v as usize]).collect();
    order.par_sort_by(|&a, &b| {
        inp.score[b as usize]
            .total_cmp(&inp.score[a as usize])
            .then(a.cmp(&b))
    });
    let mut rank = vec![u32::MAX; n];
    for (r, &v) in order.iter().enumerate() {
        rank[v as usize] = r as u32;
    }
    let zmax = choose_zmax(inp, &order, budget);
    let concepts_on = inp.has_concepts();
    let floor: Vec<u8> = (0..n)
        .map(|v| {
            let f = if !concepts_on || inp.general[v] {
                0
            } else if inp.concept_of[v] != atlas_core::NONE
                || inp.snap.node_type(v as u32) == atlas_core::EntityType::Resource
            {
                budget.detail_zoom
            } else {
                budget.general_zoom + 1
            };
            f.min(zmax) as u8
        })
        .collect();
    let minzoom = min_zooms(inp, &order, zmax, budget, &floor);
    let general_zoom = concepts_on.then_some(budget.general_zoom);
    let display_level: Vec<Option<u32>> = (0..=zmax)
        .map(|z| {
            if z == zmax || inp.levels.is_empty() {
                return None;
            }
            let cap = budget.clusters as f64 * 4f64.powi(z as i32);
            let fit = (0..inp.levels.len())
                .rev()
                .find(|&l| inp.counts[l] as f64 <= cap);
            Some(fit.unwrap_or(0) as u32)
        })
        .collect();
    let mut index = Vec::new();
    let mut blob = Vec::new();
    let mut zooms = Vec::new();
    for z in 0..=zmax {
        let t = std::time::Instant::now();
        let general = general_zoom.is_some_and(|g| z <= g);
        let (tiles, mut st) = build_zoom(
            inp,
            budget,
            &rank,
            &minzoom,
            z,
            display_level[z as usize],
            general,
        );
        let mut sizes: Vec<u32> = tiles.iter().map(|t| t.1.len() as u32).collect();
        sizes.sort_unstable();
        let pct = |p: f64| {
            sizes
                .get(((sizes.len() as f64 - 1.0) * p).round() as usize)
                .copied()
                .unwrap_or(0)
        };
        st.bytes_p50 = pct(0.5);
        st.bytes_p95 = pct(0.95);
        st.bytes_max = sizes.last().copied().unwrap_or(0);
        st.bytes_total = sizes.iter().map(|&s| s as u64).sum();
        for (k, b) in tiles {
            index.push([k, blob.len() as u64, b.len() as u64]);
            blob.extend_from_slice(&b);
        }
        st.secs = t.elapsed().as_secs_f64();
        zooms.push(st);
    }
    let meta = TileMeta {
        zmax,
        display_level,
        budget: budget.clone(),
        count: index.len(),
        zooms,
        general_zoom,
        general_nodes: inp.general.iter().filter(|&&g| g).count(),
    };
    TileSet {
        meta,
        index,
        blob: Bytes::Owned(blob),
    }
}

/// Count active nodes per tile at zoom z; returns the fullest tile's count.
fn max_per_tile(inp: &Input, order: &[u32], z: u32) -> usize {
    let mut keys: Vec<u64> = order
        .par_iter()
        .map(|&v| {
            let (x, y) = cell(inp.pos[v as usize], z);
            (x as u64) << 32 | y as u64
        })
        .collect();
    keys.par_sort_unstable();
    let (mut best, mut run) = (0usize, 0usize);
    for i in 0..keys.len() {
        run = if i > 0 && keys[i] == keys[i - 1] {
            run + 1
        } else {
            1
        };
        best = best.max(run);
    }
    best
}

fn choose_zmax(inp: &Input, order: &[u32], budget: &TileBudget) -> u32 {
    (0..budget.max_zoom)
        .find(|&z| max_per_tile(inp, order, z) <= budget.full as usize)
        .unwrap_or(budget.max_zoom)
}

/// Greedy minimum zoom per node, in priority order (see module doc), never below
/// its `floor`.  An entity fits while its tile holds fewer than `entities` visible
/// nodes of any kind; a general-set node while it holds fewer than
/// `general_tile_nodes` general nodes (so entities never crowd the overview out).
fn min_zooms(inp: &Input, order: &[u32], zmax: u32, budget: &TileBudget, floor: &[u8]) -> Vec<u8> {
    let mut minzoom = vec![u8::MAX; inp.pos.len()];
    // [all visible nodes, visible general nodes] per zoom and cell.
    let mut counts: [Vec<AHashMap<u64, u32>>; 2] =
        [0, 1].map(|_| (0..zmax).map(|_| AHashMap::new()).collect());
    let mut cells = vec![0u64; zmax as usize];
    for &v in order {
        let p = inp.pos[v as usize];
        let general = inp.general[v as usize];
        let (which, k) = if general { (1, budget.general_tile_nodes) } else { (0, budget.entities) };
        let mut first = 0u32;
        for z in 0..zmax {
            let (x, y) = cell(p, z);
            cells[z as usize] = (x as u64) << 32 | y as u64;
            if counts[which][z as usize].get(&cells[z as usize]).copied().unwrap_or(0) >= k {
                first = z + 1;
            }
        }
        first = first.max(floor[v as usize] as u32);
        for z in first..zmax {
            *counts[0][z as usize].entry(cells[z as usize]).or_insert(0) += 1;
            if general {
                *counts[1][z as usize].entry(cells[z as usize]).or_insert(0) += 1;
            }
        }
        minzoom[v as usize] = first as u8;
    }
    minzoom
}

/// One aggregated edge: representatives (CLUSTER_BIT marks a community), relation, state, count.
#[derive(Clone, Copy)]
struct AggEdge {
    a: u32,
    b: u32,
    rel: u8,
    state: u8,
    count: u32,
}

fn build_zoom(
    inp: &Input,
    budget: &TileBudget,
    rank: &[u32],
    minzoom: &[u8],
    z: u32,
    level: Option<u32>,
    general: bool,
) -> (Vec<(u64, Vec<u8>)>, ZoomStats) {
    let n = inp.pos.len();
    let lv = level.map(|l| &inp.levels[l as usize]);
    // Visible node, else its visible primary concept, else (general layer) nothing,
    // else its community at the display level.
    let rep = |v: usize| -> u32 {
        if minzoom[v] as u32 <= z {
            return v as u32;
        }
        let c = inp.concept_of[v];
        if c != atlas_core::NONE && minzoom[c as usize] as u32 <= z {
            c
        } else if general {
            UNPLACED
        } else {
            CLUSTER_BIT | lv.expect("hidden nodes exist only below zmax")[v]
        }
    };
    let membership = atlas_core::RelationType::MemberOfConcept.code();
    // Aggregate edges: sort packed keys, count runs.
    let out = inp.snap.csr(Dir::Out);
    let mut keys: Vec<u128> = (0..n.div_ceil(CHUNK))
        .into_par_iter()
        .flat_map_iter(|c| {
            let mut ks = Vec::new();
            for u in c * CHUNK..((c + 1) * CHUNK).min(n) {
                if !inp.active[u] {
                    continue;
                }
                let a = rep(u);
                for i in out.range(u as u32) {
                    let v = out.nbr[i] as usize;
                    if !inp.active[v] || v == u || out.rel[i] == membership {
                        continue;
                    }
                    // An unrepresented endpoint goes second (see the module doc).
                    let (a, b) = match rep(v) {
                        b if a == UNPLACED => (b, a),
                        b => (a, b),
                    };
                    ks.push(
                        (a as u128) << 64
                            | (b as u128) << 32
                            | (out.rel[i] as u128) << 8
                            | out.state[i] as u128,
                    );
                }
            }
            ks
        })
        .collect();
    keys.par_sort_unstable();
    let mut edges: Vec<AggEdge> = Vec::new();
    // Keyed by representative (CLUSTER_BIT kept): a cluster's or a concept's.
    let mut internal: AHashMap<u32, Vec<(u8, u32)>> = AHashMap::new();
    // (representative, relation, count) of edges whose other end is not represented.
    let mut halves: Vec<(u32, u8, u32)> = Vec::new();
    let mut edges_unplaced = 0u64;
    let mut i = 0;
    while i < keys.len() {
        let mut j = i + 1;
        while j < keys.len() && keys[j] == keys[i] {
            j += 1;
        }
        let k = keys[i];
        let e = AggEdge {
            a: (k >> 64) as u32,
            b: (k >> 32) as u32,
            rel: (k >> 8) as u8,
            state: k as u8,
            count: (j - i) as u32,
        };
        if e.a == UNPLACED {
            edges_unplaced += e.count as u64;
        } else if e.b == UNPLACED {
            match halves.last_mut() {
                Some(h) if h.0 == e.a && h.1 == e.rel => h.2 += e.count,
                _ => halves.push((e.a, e.rel, e.count)),
            }
        } else if e.a == e.b {
            let list = internal.entry(e.a).or_default();
            match list.last_mut() {
                Some(last) if last.0 == e.rel => last.1 += e.count,
                _ => list.push((e.rel, e.count)),
            }
        } else {
            edges.push(e);
        }
        i = j;
    }
    drop(keys);
    let pos_of = |r: u32| -> [f32; 2] {
        if r & CLUSTER_BIT != 0 {
            inp.centroids[level.unwrap() as usize][(r & !CLUSTER_BIT) as usize]
        } else {
            inp.pos[r as usize]
        }
    };
    let tkey = |r: u32| {
        let (x, y) = cell(pos_of(r), z);
        key(z, x, y)
    };
    // Items per tile.
    let mut ents: Vec<(u64, u32, u32)> = (0..n)
        .into_par_iter()
        .filter(|&v| inp.active[v] && minzoom[v] as u32 <= z)
        .map(|v| (tkey(v as u32), rank[v], v as u32))
        .collect();
    ents.par_sort_unstable();
    let mut hidden = vec![
        0u32;
        if general {
            0
        } else {
            level.map_or(0, |l| inp.counts[l as usize])
        }
    ];
    // Hidden members each visible concept stands for.
    let mut hidden_ent: AHashMap<u32, u32> = AHashMap::new();
    let mut unplaced = 0usize;
    for (v, &mz) in minzoom.iter().enumerate() {
        if !inp.active[v] || mz as u32 <= z {
            continue;
        }
        match rep(v) {
            UNPLACED => unplaced += 1,
            r if r & CLUSTER_BIT != 0 => hidden[(r & !CLUSTER_BIT) as usize] += 1,
            r => *hidden_ent.entry(r).or_insert(0) += 1,
        }
    }
    let mut clus: Vec<(u64, u32, u32)> = (0..hidden.len() as u32)
        .filter(|&c| hidden[c as usize] > 0)
        .map(|c| (tkey(CLUSTER_BIT | c), u32::MAX - hidden[c as usize], c))
        .collect();
    clus.sort_unstable();
    // Edge records per tile, largest counts first.
    let mut recs: Vec<u128> = edges
        .par_iter()
        .enumerate()
        .flat_map_iter(|(i, e)| {
            let (ta, tb) = (tkey(e.a), tkey(e.b));
            let r = |t: u64| (t as u128) << 64 | ((u32::MAX - e.count) as u128) << 32 | i as u128;
            std::iter::once(r(ta)).chain((tb != ta).then(|| r(tb)))
        })
        .collect();
    recs.par_sort_unstable();
    let mut halves: Vec<(u64, u8, u32)> = halves
        .into_iter()
        .map(|(r, rel, c)| (tkey(r), rel, c))
        .collect();
    halves.sort_unstable();
    // Group by tile key.
    let mut tiles: Vec<u64> = ents
        .iter()
        .map(|e| e.0)
        .chain(clus.iter().map(|c| c.0))
        .chain(recs.iter().map(|r| (r >> 64) as u64))
        .collect();
    tiles.par_sort_unstable();
    tiles.dedup();
    let range = |t: u64, keys: &dyn Fn(usize) -> u64, len: usize| {
        let lo = partition_point(len, |i| keys(i) < t);
        let hi = partition_point(len, |i| keys(i) <= t);
        lo..hi
    };
    let ctx = ZoomCtx {
        inp,
        level,
        edge_cap: if general {
            budget.general_edges
        } else {
            budget.edges
        } as usize,
        internal: &internal,
        hidden: &hidden,
        hidden_ent: &hidden_ent,
        edges: &edges,
        pos_of: &pos_of,
        tkey: &tkey,
    };
    let out: Vec<(u64, Vec<u8>, u32, u32, bool)> = tiles
        .par_iter()
        .map(|&t| {
            let er = range(t, &|i| ents[i].0, ents.len());
            let cr = range(t, &|i| clus[i].0, clus.len());
            let rr = range(t, &|i| (recs[i] >> 64) as u64, recs.len());
            let rec_idx: Vec<u32> = recs[rr].iter().map(|r| *r as u32).collect();
            let hr = range(t, &|i| halves[i].0, halves.len());
            let (b, ne, nedges, dropped) =
                encode(&ctx, t, &ents[er], &clus[cr], &rec_idx, &halves[hr]);
            (t, b, ne, nedges, dropped)
        })
        .collect();
    let st = ZoomStats {
        z,
        display_level: level,
        tiles: out.len(),
        visible_entities: ents.len(),
        clusters: clus.len(),
        aggregated_edges: edges.len(),
        max_entities: out.iter().map(|o| o.2).max().unwrap_or(0),
        max_edges: out.iter().map(|o| o.3).max().unwrap_or(0),
        tiles_with_hidden_edges: out.iter().filter(|o| o.4).count(),
        concepts: ents
            .iter()
            .filter(|e| inp.snap.concept_index(e.2).is_some())
            .count(),
        represented_by_concept: hidden_ent.values().map(|&h| h as usize).sum(),
        unplaced,
        edges_unplaced,
        ..Default::default()
    };
    (out.into_iter().map(|o| (o.0, o.1)).collect(), st)
}

fn partition_point(len: usize, pred: impl Fn(usize) -> bool) -> usize {
    let (mut lo, mut hi) = (0, len);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if pred(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

struct ZoomCtx<'a> {
    inp: &'a Input<'a>,
    level: Option<u32>,
    edge_cap: usize,
    internal: &'a AHashMap<u32, Vec<(u8, u32)>>,
    hidden: &'a [u32],
    hidden_ent: &'a AHashMap<u32, u32>,
    edges: &'a [AggEdge],
    pos_of: &'a (dyn Fn(u32) -> [f32; 2] + Sync),
    tkey: &'a (dyn Fn(u32) -> u64 + Sync),
}

fn quant(v: f32, lo: f64, size: f64) -> u16 {
    (((v as f64 - lo) / size * 65536.0)
        .floor()
        .clamp(0.0, 65535.0)) as u16
}

fn encode(
    ctx: &ZoomCtx,
    t: u64,
    ents: &[(u64, u32, u32)],
    clus: &[(u64, u32, u32)],
    recs: &[u32],
    halves: &[(u64, u8, u32)],
) -> (Vec<u8>, u32, u32, bool) {
    let (z, x, y) = (
        (t >> 48) as u32,
        ((t >> 24) & 0xFF_FFFF) as u32,
        (t & 0xFF_FFFF) as u32,
    );
    let size = 1.0 / (1u64 << z) as f64;
    let (x0, y0) = (x as f64 * size, y as f64 * size);
    let mut local: AHashMap<u32, u32> = AHashMap::with_capacity(ents.len() + clus.len());
    for (i, e) in ents.iter().enumerate() {
        local.insert(e.2, i as u32);
    }
    for (i, c) in clus.iter().enumerate() {
        local.insert(CLUSTER_BIT | c.2, (ents.len() + i) as u32);
    }
    // The cap applies per projection: a tile is read one layer at a time, so the
    // heavy layer (dependencies) must not crowd the others out of the tile.
    let mut per_layer = [0usize; 32];
    let (kept, over): (Vec<u32>, Vec<u32>) = recs.iter().partition(|&&i| {
        let k = &mut per_layer[projection_of(ctx.edges[i as usize].rel)];
        *k += 1;
        *k <= ctx.edge_cap
    });
    let kept = &kept[..];
    let mut ext: Vec<u32> = Vec::new();
    for &i in kept {
        let e = ctx.edges[i as usize];
        for r in [e.a, e.b] {
            if (ctx.tkey)(r) != t && !local.contains_key(&r) {
                local.insert(r, (ents.len() + clus.len() + ext.len()) as u32);
                ext.push(r);
            }
        }
    }
    let mut hidden_rel: Vec<(u8, u32)> = Vec::new();
    let over = over.iter().map(|&i| {
        let e = ctx.edges[i as usize];
        (e.rel, e.count)
    });
    for (rel, count) in over.chain(halves.iter().map(|h| (h.1, h.2))) {
        match hidden_rel.iter_mut().find(|h| h.0 == rel) {
            Some(h) => h.1 += count,
            None => hidden_rel.push((rel, count)),
        }
    }
    hidden_rel.sort_unstable();
    let mut b = Vec::with_capacity(16 + ents.len() * 10 + kept.len() * 8);
    for v in [
        ents.len(),
        clus.len(),
        ext.len(),
        kept.len(),
        hidden_rel.len(),
    ] {
        put(&mut b, v as u64);
    }
    let put_rels = |b: &mut Vec<u8>, r: u32| {
        let rels = ctx.internal.get(&r).map(|v| v.as_slice()).unwrap_or(&[]);
        put(b, rels.len() as u64);
        for &(rel, count) in rels {
            b.push(rel);
            put(b, count as u64);
        }
    };
    for e in ents {
        let p = ctx.inp.pos[e.2 as usize];
        put(&mut b, e.2 as u64);
        put(&mut b, e.1 as u64);
        b.extend_from_slice(&quant(p[0], x0, size).to_le_bytes());
        b.extend_from_slice(&quant(p[1], y0, size).to_le_bytes());
        put(
            &mut b,
            ctx.hidden_ent.get(&e.2).copied().unwrap_or(0) as u64,
        );
        put_rels(&mut b, e.2);
    }
    let l = ctx.level.map(|l| l as usize);
    for c in clus {
        let p = (ctx.pos_of)(CLUSTER_BIT | c.2);
        put(&mut b, c.2 as u64);
        b.extend_from_slice(&quant(p[0], x0, size).to_le_bytes());
        b.extend_from_slice(&quant(p[1], y0, size).to_le_bytes());
        put(&mut b, ctx.inp.sizes[l.unwrap()][c.2 as usize] as u64);
        put(&mut b, ctx.hidden[c.2 as usize] as u64);
        put_rels(&mut b, CLUSTER_BIT | c.2);
    }
    for &r in &ext {
        let p = (ctx.pos_of)(r);
        put(
            &mut b,
            ((r & !CLUSTER_BIT) as u64) << 1 | (r & CLUSTER_BIT != 0) as u64,
        );
        b.extend_from_slice(&((p[0] as f64 * 4294967295.0) as u32).to_le_bytes());
        b.extend_from_slice(&((p[1] as f64 * 4294967295.0) as u32).to_le_bytes());
    }
    for &i in kept {
        let e = ctx.edges[i as usize];
        put(&mut b, local[&e.a] as u64);
        put(&mut b, local[&e.b] as u64);
        b.push(e.rel);
        b.push(e.state);
        put(&mut b, e.count as u64);
    }
    for (rel, count) in &hidden_rel {
        b.push(*rel);
        put(&mut b, *count as u64);
    }
    (
        b,
        ents.len() as u32,
        kept.len() as u32,
        !hidden_rel.is_empty(),
    )
}

/// Projection (map layer) code of a relation code.
fn projection_of(rel: u8) -> usize {
    atlas_core::RelationType::from_code(rel).map_or(31, |r| r.spec().projection.code() as usize)
}

fn put(b: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        b.push((v as u8) | 0x80);
        v >>= 7;
    }
    b.push(v as u8);
}

/// (community, x, y, members, hidden members, [(relation code, count)])
pub type DecodedCluster = (u32, f32, f32, u32, u32, Vec<(u8, u32)>);

/// A decoded binary tile.  Positions are in the unit square.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Decoded {
    /// (node, priority rank, x, y), by rank.
    pub entities: Vec<(u32, u32, f32, f32)>,
    /// Per entity: (hidden members it stands for, [(relation code, count)] among them).
    pub entity_hidden: Vec<(u32, Vec<(u8, u32)>)>,
    /// (community, x, y, members, hidden members, [(relation code, count)])
    pub clusters: Vec<DecodedCluster>,
    /// (node or community, is_cluster, x, y)
    pub external: Vec<(u32, bool, f32, f32)>,
    /// (a, b, relation code, state code, count), local indices as in the module doc.
    pub edges: Vec<(u32, u32, u8, u8, u32)>,
    pub hidden_edges: Vec<(u8, u32)>,
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn v(&mut self) -> Result<u64, String> {
        let (mut out, mut shift) = (0u64, 0);
        loop {
            let byte = *self.b.get(self.at).ok_or("tile truncated")?;
            self.at += 1;
            out |= ((byte & 0x7F) as u64) << shift;
            if byte < 0x80 {
                return Ok(out);
            }
            shift += 7;
            if shift > 63 {
                return Err("bad varint".into());
            }
        }
    }
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], String> {
        let s = self.b.get(self.at..self.at + N).ok_or("tile truncated")?;
        self.at += N;
        Ok(s.try_into().unwrap())
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.bytes::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.bytes()?))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }
}

pub fn decode(blob: &[u8], z: u32, x: u32, y: u32) -> Result<Decoded, String> {
    let size = 1.0 / (1u64 << z) as f64;
    let (x0, y0) = (x as f64 * size, y as f64 * size);
    let deq = |q: u16, lo: f64| (lo + (q as f64 + 0.5) / 65536.0 * size) as f32;
    let mut r = Reader { b: blob, at: 0 };
    let (ne, nc, nx, nedges, nh) = (r.v()?, r.v()?, r.v()?, r.v()?, r.v()?);
    let mut d = Decoded::default();
    let rels = |r: &mut Reader| -> Result<Vec<(u8, u32)>, String> {
        let nrel = r.v()?;
        let mut out = Vec::with_capacity(nrel.min(64) as usize);
        for _ in 0..nrel {
            out.push((r.u8()?, r.v()? as u32));
        }
        Ok(out)
    };
    for _ in 0..ne {
        let (node, rank) = (r.v()? as u32, r.v()? as u32);
        let (qx, qy) = (r.u16()?, r.u16()?);
        d.entities.push((node, rank, deq(qx, x0), deq(qy, y0)));
        let hidden = r.v()? as u32;
        d.entity_hidden.push((hidden, rels(&mut r)?));
    }
    for _ in 0..nc {
        let c = r.v()? as u32;
        let (qx, qy) = (r.u16()?, r.u16()?);
        let (members, hidden) = (r.v()? as u32, r.v()? as u32);
        let internal = rels(&mut r)?;
        d.clusters
            .push((c, deq(qx, x0), deq(qy, y0), members, hidden, internal));
    }
    for _ in 0..nx {
        let rf = r.v()?;
        let (gx, gy) = (r.u32()?, r.u32()?);
        d.external.push((
            (rf >> 1) as u32,
            rf & 1 == 1,
            (gx as f64 / 4294967295.0) as f32,
            (gy as f64 / 4294967295.0) as f32,
        ));
    }
    let items = ne + nc + nx;
    for _ in 0..nedges {
        let (a, b) = (r.v()?, r.v()?);
        if a >= items || b >= items {
            return Err("edge endpoint out of range".into());
        }
        let (rel, state, count) = (r.u8()?, r.u8()?, r.v()? as u32);
        d.edges.push((a as u32, b as u32, rel, state, count));
    }
    for _ in 0..nh {
        d.hidden_edges.push((r.u8()?, r.v()? as u32));
    }
    if r.at != blob.len() {
        return Err("trailing bytes in tile".into());
    }
    Ok(d)
}
