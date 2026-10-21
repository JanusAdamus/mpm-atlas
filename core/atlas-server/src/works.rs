//! The works layer (ADR-0005 §8.4): a layer snapshot bound to the core, its lexical fragment and its
//! on-request tiles.
//!
//! | Route | Answer |
//! |---|---|
//! | `/api/works/search?q=&lang=&limit=` | atlas-search works fragment (lexical only) |
//! | `/api/map/tiles/works/{z}/{x}/{y}`, `/api/map/tiles?layer=works&z=&x=&y=` | atlas-layout works tiles |
//!
//! `compile` builds the three artifacts into one directory: `snapshot/` (`atlas-<hash>.snap` +
//! `CURRENT`), `index/` and `works-tiles-<layer hash 16>.bin`.

use atlas_core::{CompileOptions, Dir, EntityType, Error, NodeIx, RelationType, Result, Snapshot};
use atlas_layout::works::{self as wt, WorkInput, WorksTiles};
use atlas_search::works::WorksIndex;
use axum::extract::{Path as UrlPath, State};
use axum::response::Response;
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::http::{ApiError, Q, blocking, check_lang, check_limit, json_bytes, to_json_vec};
use crate::{AppState, Engine};

const MAX_QUERY_CHARS: usize = 256;

pub struct Works {
    pub snap: &'static Snapshot,
    pub index: Option<WorksIndex>,
    pub tiles: Option<WorksTiles>,
}

fn open_ro(db: &Path) -> Result<Connection> {
    let c = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(c)
}

fn is_work(t: EntityType) -> bool {
    matches!(t, EntityType::Publication | EntityType::Witness)
}

/// Tile inputs: per canonical work, the union of its date spans (`entity_dates`, every merged id),
/// the MSC top class of its area and its in-degree of corpus-internal citations.
pub fn tile_inputs(layer: &Snapshot, db: &Path) -> Result<Vec<WorkInput>> {
    let conn = open_ro(db)?;
    let mut span: HashMap<NodeIx, (Option<i32>, Option<i32>)> = HashMap::new();
    let mut st = conn.prepare("SELECT entity_id, MIN(lo), MAX(hi), SUM(lo IS NULL), SUM(hi IS NULL) FROM entity_dates GROUP BY entity_id")?;
    let mut rows = st.query([])?;
    while let Some(r) = rows.next()? {
        let Some(n) = layer.resolve(r.get_ref(0)?.as_str()?) else { continue };
        let (lo, hi): (Option<i32>, Option<i32>) = (r.get(1)?, r.get(2)?);
        let (open_lo, open_hi): (i64, i64) = (r.get(3)?, r.get(4)?);
        let e = span.entry(n).or_insert((None, None));
        let lo = if open_lo > 0 { None } else { lo };
        let hi = if open_hi > 0 { None } else { hi };
        e.0 = match (e.0, lo) { (Some(a), Some(b)) => Some(a.min(b)), (a, b) => a.or(b) };
        e.1 = match (e.1, hi) { (Some(a), Some(b)) => Some(a.max(b)), (a, b) => a.or(b) };
    }
    let mut msc: HashMap<String, u8> = HashMap::new();
    let mut st = conn.prepare(
        "SELECT x.entity_id, x.value FROM external_ids x JOIN entities e ON e.id = x.entity_id \
         WHERE x.scheme = 'msc' AND e.type = 'area' ORDER BY x.entity_id, x.value")?;
    let mut rows = st.query([])?;
    while let Some(r) = rows.next()? {
        let (id, code): (String, String) = (r.get(0)?, r.get(1)?);
        if let Ok(c) = code.get(..2).unwrap_or("").parse::<u8>() {
            msc.entry(id).or_insert(c);
        }
    }
    let inc = layer.csr(Dir::In);
    Ok((0..layer.num_nodes())
        .filter(|&n| is_work(layer.node_type(n)) && !layer.is_merged(n))
        .map(|n| {
            let (lo, hi) = span.get(&n).copied().unwrap_or((None, None));
            let class = layer.area(n).and_then(|a| msc.get(layer.node_id(a)).copied());
            WorkInput {
                node: n,
                id_hash: u64::from_le_bytes(blake3::hash(layer.node_id(n).as_bytes()).as_bytes()[..8].try_into().expect("8")),
                lo,
                hi,
                msc_class: class,
                indeg: inc.edges_rel(n, RelationType::Cites).len() as u32,
            }
        })
        .collect())
}

/// Compile the works layer over the core snapshot in `core_dir`, then its index and tile file.
pub fn compile(db: &Path, core_dir: &Path, out: &Path, threads: usize) -> Result<Value> {
    let core = Snapshot::open(core_dir)?;
    let t0 = Instant::now();
    let mut opts = CompileOptions::new(db, out.join("snapshot"));
    opts.threads = threads;
    opts.core_content_hash = Some(core.header().content_hash);
    let report = atlas_core::compile(&opts)?;
    let compile_secs = t0.elapsed().as_secs_f64();
    let layer = Snapshot::open_layer(&report.snapshot, &core)?;
    let index = atlas_search::works::build(&layer, db, &out.join("index"), threads.clamp(1, 16))?;
    let t = Instant::now();
    let inputs = tile_inputs(&layer, db)?;
    let bytes = wt::build(&inputs, wt::BUDGET);
    let path = wt::save(&bytes, out, &layer.content_hash_hex()[..16])?;
    let tiles = WorksTiles::open(&path)?;
    let tiles_secs = t.elapsed().as_secs_f64();
    Ok(json!({
        "snapshot": report, "compile_secs": compile_secs, "index": index,
        "tiles": {"path": path, "bytes": bytes.len(), "placed": tiles.len(), "undated": tiles.undated, "secs": tiles_secs},
        "total_secs": t0.elapsed().as_secs_f64(),
    }))
}

/// Open the works directory written by `compile` next to `core`.
pub fn open(dir: &Path, core: &Snapshot) -> Result<(Works, String)> {
    let snap: &'static Snapshot = Box::leak(Box::new(Snapshot::open_layer(dir.join("snapshot"), core)?));
    let index = WorksIndex::open(&dir.join("index")).ok();
    let tiles_path: PathBuf = dir.join(format!("works-tiles-{}.bin", &snap.content_hash_hex()[..16]));
    let tiles = WorksTiles::open(&tiles_path).ok();
    let status = format!(
        "ok: layer {} ({} nodes), index {}, tiles {}",
        &snap.content_hash_hex()[..16],
        snap.num_nodes(),
        index.as_ref().map_or("missing".to_string(), |i| format!("{} docs", i.num_docs())),
        tiles.as_ref().map_or("missing".to_string(), |t| format!("{} placed", t.len()))
    );
    Ok((Works { snap, index, tiles }, status))
}

// ---------------------------------------------------------------------- routes

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    q: String,
    lang: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
struct WorkHit<'a> {
    id: &'a str,
    label: &'a str,
    #[serde(rename = "type")]
    ty: &'static str,
    score: f32,
}

pub async fn search(State(st): State<AppState>, Q(p): Q<SearchParams>) -> std::result::Result<Response, ApiError> {
    let q = p.q.trim().to_string();
    if q.is_empty() || q.chars().count() > MAX_QUERY_CHARS {
        return Err(ApiError::bad_request(format!("q must have 1..={MAX_QUERY_CHARS} characters")));
    }
    let lang = p.lang.unwrap_or_else(|| "es".into());
    check_lang(&lang)?;
    let limit = check_limit(p.limit, 20, 100)?;
    let body = blocking(&st.engine, move |e| {
        let w = e.works()?;
        let idx = w.index.as_ref().ok_or(Error::NotImplemented("works search: no works index"))?;
        let hits = idx.search(&q, &lang, limit)?;
        let out: Vec<WorkHit> = hits
            .iter()
            .map(|&(n, score)| WorkHit { id: w.snap.node_id(n), label: w.snap.label(n, &lang), ty: w.snap.node_type(n).as_str(), score })
            .collect();
        to_json_vec(&json!({"q": q, "lane": "works-lexical", "hits": out}))
    })
    .await?;
    Ok(json_bytes(body))
}

#[derive(Serialize)]
struct TileNode<'a> {
    id: &'a str,
    label: &'a str,
    #[serde(rename = "type")]
    ty: &'static str,
    x: f32,
    y: f32,
    year_lo: i32,
    year_hi: i32,
    internal_citations: u32,
}

pub async fn tile(st: State<AppState>, UrlPath((z, x, y)): UrlPath<(u32, u32, u32)>) -> std::result::Result<Response, ApiError> {
    tile_json(st, z, x, y, "es".into()).await
}

pub(crate) async fn tile_json(State(st): State<AppState>, z: u32, x: u32, y: u32, lang: String) -> std::result::Result<Response, ApiError> {
    let body = blocking(&st.engine, move |e| {
        let w = e.works()?;
        let tiles = w.tiles.as_ref().ok_or(Error::NotImplemented("works tiles: no works tile file"))?;
        let t = tiles.tile(z, x, y)?;
        let nodes: Vec<TileNode> = t
            .shown
            .iter()
            .map(|p| TileNode {
                id: w.snap.node_id(p.node),
                label: w.snap.label(p.node, &lang),
                ty: w.snap.node_type(p.node).as_str(),
                x: p.x,
                y: p.y,
                year_lo: p.lo,
                year_hi: p.hi,
                internal_citations: p.indeg,
            })
            .collect();
        to_json_vec(&json!({
            "layer": "works", "z": z, "x": x, "y": y, "in_tile": t.in_tile, "shown": nodes.len(),
            "budget": tiles.budget, "undated_not_placed": tiles.undated,
            "order": "citations internal to the ingested corpus (not a measure of importance)",
            "nodes": nodes,
        }))
    })
    .await?;
    Ok(json_bytes(body))
}

impl Engine {
    pub(crate) fn works(&self) -> Result<&Works> {
        self.works.as_ref().ok_or(Error::NotImplemented("works layer: not loaded (--works)"))
    }
}
