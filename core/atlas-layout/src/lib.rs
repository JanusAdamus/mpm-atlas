//! atlas-layout: the multiscale map (WP08).
//!
//! 1. A weighted undirected graph from the snapshot's current claims, restricted
//!    to the relations of `LayoutParams::layers` (`graph`).
//! 2. Leiden communities (R37) at several resolutions, nested coarse to fine
//!    (area ⊇ region ⊇ neighbourhood), with modularity / CPM reported (`leiden`).
//!    Communities are exploratory groupings, not canonical areas of mathematics.
//! 3. A multilevel ForceAtlas2 layout with Barnes-Hut repulsion, coarsened by
//!    those communities, computed in parallel (`force`, `multilevel`).  Positions
//!    persist with a hash of each node id; a rebuild warm-starts from them so
//!    existing nodes barely move.
//! 4. Precomputed z/x/y tiles in a compact binary format: visible entities by
//!    label priority, clusters for the rest, and aggregated edges that keep one
//!    relation and one evidence state per line with their counts (`tiles`).
//!
//! Nodes with no relation in the chosen layers are grouped in one community per
//! level whose label says their relations are unknown (principle 7), never
//! "isolated".  Merged nodes take their survivor's place and never show on tiles.
//!
//! Concept layer (when the snapshot has one): each declaration is drawn in a disc
//! around its primary concept; the general zooms show only concepts and areas, and
//! a visible concept stands for its hidden declarations (`concept`, `tiles`).
//!
//! Positions are stable within a `layout_version`.  An aggregated edge carries one
//! relation and its count: distinct relations are never merged into one line.

mod concept;
pub mod force;
pub mod graph;
mod io;
pub mod leiden;
mod multilevel;
mod pipeline;
pub mod tiles;
mod util;
pub mod works;

use ahash::AHashMap;
use atlas_core::{
    EntityType, Error, EvidenceState, NONE, Projection, RelationType, Result, Snapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use force::ForceParams;
pub use leiden::Quality;
pub use multilevel::LevelRun;
pub use tiles::{Decoded, TileBudget, TileMeta, TileSet, ZoomStats};

#[derive(Clone, Debug)]
pub struct LayoutParams {
    /// Leiden resolutions, coarse to fine (increasing); one community level per value.
    pub resolutions: Vec<f64>,
    /// Cap on a coarse level's largest community, as a share of the nodes with
    /// relations; a level over it gets a higher resolution (`leiden::hierarchy_seeded`).
    /// 1.0 turns the cap off.
    pub max_share: f64,
    pub quality: Quality,
    /// Full Leiden passes per level (each starts from the previous partition).
    pub leiden_iterations: u32,
    /// ForceAtlas2 iterations on the coarsest level.
    pub iterations: u32,
    /// Iterations on each finer level, nodes included.
    pub refine_iterations: u32,
    /// Iterations of a warm-started rebuild (nodes only).
    pub warm_iterations: u32,
    pub seed: u64,
    /// Relations (by projection) that pull nodes together.
    pub layers: Vec<Projection>,
    pub force: ForceParams,
    pub tiles: TileBudget,
}

impl Default for LayoutParams {
    fn default() -> Self {
        LayoutParams {
            resolutions: vec![0.1, 1.0, 10.0],
            max_share: 0.25,
            quality: Quality::Modularity,
            leiden_iterations: 2,
            iterations: 300,
            refine_iterations: 60,
            warm_iterations: 40,
            seed: 42,
            layers: Projection::ALL.to_vec(),
            force: ForceParams::default(),
            tiles: TileBudget::default(),
        }
    }
}

/// One community level.  The vectors are indexed by community.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelInfo {
    /// The resolution asked for (the level's identity for warm starts).
    pub resolution: f64,
    /// The resolution used, higher than `resolution` when `max_share` raised it.
    #[serde(default)]
    pub resolution_used: f64,
    pub count: usize,
    /// Standard modularity (γ = 1) on the layout graph.
    pub modularity: f64,
    /// The optimised objective at `resolution` (Q_γ, or CPM's H).
    pub objective: f64,
    /// The community of nodes with no relation in the layers ("unknown"), if any.
    pub unknown: Option<u32>,
    pub largest: u32,
    pub median_size: u32,
    pub singletons: usize,
    pub leiden_secs: f64,
    #[serde(skip)]
    pub sizes: Vec<u32>,
    #[serde(skip)]
    pub centroids: Vec<[f32; 2]>,
    /// Up to three most central members (internal weighted degree), `NONE`-padded.
    #[serde(skip)]
    pub central: Vec<[u32; 3]>,
}

impl LevelInfo {
    fn summary(&self) -> LevelInfo {
        LevelInfo {
            sizes: Vec::new(),
            centroids: Vec::new(),
            central: Vec::new(),
            ..self.clone()
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WarmStats {
    pub prior_version: String,
    /// Active nodes that had a previous position.
    pub matched: usize,
    pub new: usize,
    /// Per level: share of matched nodes (with relations) still grouped with the
    /// majority of their previous community.  Levels whose resolution changed get none.
    #[serde(default)]
    pub communities_kept: Vec<Option<f64>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LayoutStats {
    pub nodes: usize,
    pub active: usize,
    pub merged: usize,
    /// Active nodes with no relation in the layers.
    pub unknown: usize,
    pub layout_edges: usize,
    pub secs_graph: f64,
    pub secs_leiden: f64,
    pub secs_layout: f64,
    pub secs_tiles: f64,
    pub secs_total: f64,
    pub runs: Vec<LevelRun>,
    pub warm: Option<WarmStats>,
    /// Why a prior in the output directory was not used, if it was not.
    pub prior_error: Option<String>,
    /// Mean edge length inside / between finest communities (unit square).
    pub edge_len_intra: f64,
    pub edge_len_inter: f64,
}

/// A computed layout, keyed by the snapshot it was built from.
pub struct Layout {
    /// Hash of (snapshot content hash, params, prior version): what `/api/map/tiles` reports.
    pub version: String,
    pub snapshot_hash: String,
    /// Node positions in [0,1]^2, indexed by snapshot node (NaN for excluded nodes):
    /// what the map draws, declarations around their concept.
    pub positions: Vec<[f32; 2]>,
    /// The force layout's positions, before the concept placement: what a warm
    /// start continues from (equal to `positions` without a concept layer).
    pub force_positions: Vec<[f32; 2]>,
    /// Community of each node per level (`levels[l][node]`), coarse to fine.
    pub levels: Vec<Vec<u32>>,
    pub communities: Vec<LevelInfo>,
    /// `atlas_core::hashidx::key_hash` of each node id: the key of a warm start.
    pub id_hash: Vec<u64>,
    /// Layout coordinates → unit square: `p · affine[0] + (affine[1], affine[2])`.
    pub affine: [f64; 3],
    pub stats: LayoutStats,
    pub tiles: TileSet,
}

#[derive(Clone, Debug, Serialize)]
pub struct LayoutReport {
    pub version: String,
    pub dir: PathBuf,
    pub nodes: u32,
    pub levels: usize,
    pub secs: f64,
    pub layout_file: PathBuf,
    pub tiles_file: PathBuf,
    pub layout_bytes: u64,
    pub tiles_bytes: u64,
    pub tiles: usize,
    pub zmax: u32,
    pub communities: Vec<LevelInfo>,
    pub stats: LayoutStats,
}

/// A previous layout, keyed by node id hash, for a warm start: positions for the
/// force layout and communities to seed Leiden.
pub struct Prior {
    pub version: String,
    pub affine: [f64; 3],
    /// Node id hash → node index in the previous layout.
    pub(crate) map: AHashMap<u64, u32>,
    pub(crate) positions: Vec<[f32; 2]>,
    /// (resolution asked for, resolution used, membership) per level, as in `Layout::levels`.
    pub(crate) levels: Vec<(f64, f64, Vec<u32>)>,
    /// Nodes that sat in the "unknown" group (no relation in the layers).
    pub(crate) unknown: Vec<bool>,
}

impl Prior {
    pub fn from_layout(l: &Layout) -> Prior {
        let map = (0..l.positions.len() as u32)
            .filter(|&v| l.positions[v as usize][0].is_finite())
            .map(|v| (l.id_hash[v as usize], v))
            .collect();
        let levels = l
            .communities
            .iter()
            .zip(&l.levels)
            .map(|(c, m)| (c.resolution, c.resolution_used, m.clone()))
            .collect();
        let unknown = match (l.levels.last(), l.communities.last()) {
            (Some(m), Some(info)) => m.iter().map(|&c| Some(c) == info.unknown).collect(),
            _ => vec![false; l.positions.len()],
        };
        Prior {
            version: l.version.clone(),
            affine: l.affine,
            map,
            positions: l.force_positions.clone(),
            levels,
            unknown,
        }
    }

    /// The current layout in `dir`, if there is one.
    pub fn open(dir: &Path) -> Result<Option<Prior>> {
        match io::current(dir)? {
            None => Ok(None),
            Some(v) => Ok(Some(Prior::from_layout(&io::read_layout(dir, &v)?))),
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[derive(Default)]
pub struct ComputeOptions {
    pub prior: Option<Prior>,
    /// Nodes treated as absent (benchmarks of incremental rebuilds).
    pub exclude: Option<Vec<bool>>,
    /// Skip tile building (benchmarks of the layout alone).
    pub skip_tiles: bool,
}

pub fn compute(snapshot: &Snapshot, params: &LayoutParams) -> Result<Layout> {
    compute_with(snapshot, params, &ComputeOptions::default())
}

pub fn compute_with(
    snapshot: &Snapshot,
    params: &LayoutParams,
    opts: &ComputeOptions,
) -> Result<Layout> {
    pipeline::run(snapshot, params, opts)
}

/// Compute, warm-starting from the current layout in `dir` if any, and write.
pub fn build(snapshot: &Snapshot, params: &LayoutParams, dir: &Path) -> Result<LayoutReport> {
    let (prior, prior_error) = match Prior::open(dir) {
        Ok(p) => (p, None),
        Err(e) => (None, Some(e.to_string())),
    };
    let mut layout = compute_with(
        snapshot,
        params,
        &ComputeOptions {
            prior,
            ..Default::default()
        },
    )?;
    layout.stats.prior_error = prior_error;
    layout.write(dir)
}

impl Layout {
    pub fn write(&self, dir: &Path) -> Result<LayoutReport> {
        let (lp, tp) = io::write(self, dir)?;
        let len = |p: &Path| {
            std::fs::metadata(p)
                .map(|m| m.len())
                .map_err(|e| Error::io(p, e))
        };
        Ok(LayoutReport {
            version: self.version.clone(),
            dir: dir.to_path_buf(),
            nodes: self.positions.len() as u32,
            levels: self.levels.len(),
            secs: self.stats.secs_total,
            layout_bytes: len(&lp)?,
            tiles_bytes: len(&tp)?,
            layout_file: lp,
            tiles_file: tp,
            tiles: self.tiles.len(),
            zmax: self.tiles.meta.zmax,
            communities: self.communities.iter().map(LevelInfo::summary).collect(),
            stats: self.stats.clone(),
        })
    }

    /// The current layout in `dir`; it must have been built from `snapshot`.
    pub fn open(snapshot: &Snapshot, dir: &Path) -> Result<Layout> {
        let v = io::current(dir)?
            .ok_or_else(|| Error::NotFound(format!("no layout in {}", dir.display())))?;
        let mut l = io::read_layout(dir, &v)?;
        let want = snapshot.content_hash_hex();
        if l.snapshot_hash != want || l.positions.len() != snapshot.num_nodes() as usize {
            return Err(Error::Integrity(format!(
                "layout {v} was built from snapshot {}, not {want}; rebuild it with `atlas layout`",
                l.snapshot_hash
            )));
        }
        l.tiles = io::read_tiles(dir, &v)?;
        Ok(l)
    }

    /// "L<level>:<community>", or "L<level>:unknown" for the unknown group.
    pub fn community_id(&self, level: usize, c: u32) -> String {
        if self.communities[level].unknown == Some(c) {
            format!("L{level}:unknown")
        } else {
            format!("L{level}:{c}")
        }
    }

    /// A region's label: its most central member's label.
    pub fn region_label(&self, snapshot: &Snapshot, level: usize, c: u32, lang: &str) -> String {
        let info = &self.communities[level];
        if info.unknown == Some(c) {
            return if lang == "es" {
                "Relaciones desconocidas"
            } else {
                "Unknown relations"
            }
            .to_string();
        }
        match info.central.get(c as usize).map(|m| m[0]) {
            Some(n) if n != NONE => snapshot.display_label(n, lang).to_string(),
            _ => self.community_id(level, c),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TileNodeKind {
    Entity,
    Cluster,
    /// An entity that is a concept of the snapshot (it has member declarations).
    Concept,
}

#[derive(Clone, Debug, Serialize)]
pub struct TileNode {
    pub id: String,
    pub kind: TileNodeKind,
    pub label: String,
    pub x: f32,
    pub y: f32,
    /// 1 for an entity; members it stands for (hidden at this zoom) for a cluster;
    /// primary member declarations (all of them, at every zoom) for a concept.
    pub size: u32,
    pub community: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub entity_type: Option<EntityType>,
    /// Label priority (0 = most important), entities only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    /// Ids of the most central members, clusters only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub central: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct TileEdge {
    pub source: String,
    pub target: String,
    pub relation: RelationType,
    pub evidence_state: EvidenceState,
    pub count: u32,
}

/// What a cluster or a concept stands for at this zoom.
#[derive(Clone, Debug, Serialize)]
pub struct Aggregate {
    /// The cluster's or the concept's id.
    pub cluster_id: String,
    pub members: u32,
    pub hidden_members: u32,
    /// Edges between hidden members of this cluster (or concept), per relation.
    pub hidden_relations: BTreeMap<RelationType, u32>,
}

/// `GET /api/map/tiles?layer=structure&z=0&x=0&y=0`.
#[derive(Clone, Debug, Serialize)]
pub struct TileResponse {
    pub layer: Projection,
    pub layout_version: String,
    pub z: u32,
    pub x: u32,
    pub y: u32,
    pub max_zoom: u32,
    pub bbox: [f32; 4],
    pub nodes: Vec<TileNode>,
    /// Endpoints of this tile's edges that lie in other tiles.
    pub external: Vec<TileNode>,
    pub edges: Vec<TileEdge>,
    pub aggregates: Vec<Aggregate>,
    /// Edges over the per-tile budget, per relation (counted, not drawn).
    pub hidden_edges: BTreeMap<RelationType, u32>,
}

pub fn tile(
    snapshot: &Snapshot,
    layout: &Layout,
    layer: Projection,
    z: u32,
    x: u32,
    y: u32,
    lang: &str,
) -> Result<TileResponse> {
    pipeline::tile(snapshot, layout, layer, z, x, y, lang)
}
