//! atlas-server: axum JSON API (docs/SPEC.md §15) over the memory-mapped snapshot,
//! plus the built web client (web/dist).
//!
//! | Route | Source of the answer |
//! |---|---|
//! | `/api/health` | what is loaded, which lane serves each endpoint |
//! | `/api/search` | atlas-search if its index opens, else the registry FTS5 lane (`Store.search`) |
//! | `/api/entity/{id}` | snapshot (identity, merges, coverage, counts) + registry (full text, claims) |
//! | `/api/neighborhood` | snapshot CSR only |
//! | `/api/path` | atlas-paths over the snapshot |
//! | `/api/map/tiles`, `/api/map/tiles/{z}/{x}/{y}` | atlas-layout; JSON or binary (`format=bin`) |
//! | `/api/map/locate?id=` | atlas-layout: position and communities of one entity (region focus) |
//! | `/api/coverage` | snapshot, computed once at start |
//! | `/api/concept?id=`, `/api/concept/edges?from=&to=` | snapshot concept layer + registry (membership evidence) |
//!
//! Snapshot-derived routes carry a weak ETag (snapshot hash, layout version, URI) and
//! answer `If-None-Match` with 304 before doing any work.  Responses are gzip/brotli
//! compressed on request; static files are served traversal-safe, precompressed when
//! `.br`/`.gz` siblings exist.  CORS is off unless an origin is configured.

mod concept;
mod coverage;
mod entity;
mod graph;
mod http;
mod paths;
mod registry;
mod search;
mod tiles;

pub use coverage::now_utc;
pub use http::ApiError;
pub use search::registry_search;
pub use tiles::{MAGIC as TILE_MAGIC, TILE_MIME, encode_tile};

use atlas_core::{Error, Result, Snapshot};
use axum::Router;
use axum::http::{HeaderValue, Method};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::routing::get;
use axum::serve::ListenerExt;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_BODY_BYTES: usize = 1024;
/// Brotli / gzip quality for dynamic responses.  The default leaves brotli at 11,
/// its slowest setting: depth-2 neighborhoods (~200 KB of JSON) ran at half the
/// uncompressed throughput.  4 is the usual choice for on-the-fly compression.
const COMPRESSION_QUALITY: i32 = 4;
/// Blocking threads beyond the heavy permits, for static file reads.
const FS_THREADS: usize = 8;

/// What `atlas serve` loads.  Only the snapshot is required; every other part is
/// optional and `/api/health` reports why it is missing.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub snapshot: PathBuf,
    /// Registry, opened read-only (detail views, lexical fallback).
    pub db: Option<PathBuf>,
    pub web_dir: PathBuf,
    /// atlas-search index directory.
    pub index_dir: Option<PathBuf>,
    /// atlas-layout output directory.
    pub layout_dir: Option<PathBuf>,
    /// shapes/composition.json for atlas-paths.
    pub composition: Option<PathBuf>,
    /// Allowed CORS origin; `None` = no CORS headers (same-origin only).
    pub cors_origin: Option<String>,
    /// Concurrent heavy requests (search, entity, path, tiles); 0 = 2 × cores.
    pub heavy_permits: usize,
}

impl ServerConfig {
    pub fn new(snapshot: impl Into<PathBuf>) -> Self {
        ServerConfig {
            snapshot: snapshot.into(),
            db: None,
            web_dir: PathBuf::from("web/dist"),
            index_dir: None,
            layout_dir: None,
            composition: None,
            cors_origin: None,
            heavy_permits: 0,
        }
    }

    /// Concurrent heavy requests: `heavy_permits`, or 2 x cores when 0.
    pub fn permits(&self) -> usize {
        if self.heavy_permits > 0 {
            self.heavy_permits
        } else {
            2 * std::thread::available_parallelism().map_or(4, |n| n.get())
        }
    }

    /// The tokio runtime for `serve`.  The blocking pool is capped at `permits()` plus
    /// `FS_THREADS` for static files (tokio::fs): heavy work never runs wider than the
    /// permits, and tokio's default cap (512) let the pool
    /// grow to ~510 threads under load, each holding its own registry connection and
    /// page cache (measured: 545 threads, 5.2 GB private after 32 s of mixed load).
    pub fn runtime(&self) -> Result<tokio::runtime::Runtime> {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .max_blocking_threads(self.permits() + FS_THREADS)
            .build()
            .map_err(|e| Error::io("tokio runtime", e))
    }
}

/// Everything loaded for the life of the process.
pub struct Engine {
    /// Leaked on purpose: lives as long as the process, and lets atlas-paths hold
    /// `PathEngine<'static>`.  ponytail: hot snapshot reload would need an
    /// `ArcSwap<Engine>` and dropping old maps after in-flight requests finish.
    pub snap: &'static Snapshot,
    pub db: Option<PathBuf>,
    pub search: Option<atlas_search::SearchIndex>,
    pub paths: Option<atlas_paths::PathEngine<'static>>,
    pub layout: Option<atlas_layout::Layout>,
    /// Component -> "ok: …" or "unavailable: …", for `/api/health`.
    pub components: BTreeMap<&'static str, String>,
    pub coverage: serde_json::Value,
    pub started_at: String,
    pub(crate) etag_seed: [u8; 32],
    pub(crate) heavy: Arc<Semaphore>,
}

/// Shared, read-only state of the server.
#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub web_dir: PathBuf,
    pub cors_origin: Option<HeaderValue>,
}

fn optional<T>(components: &mut BTreeMap<&'static str, String>, name: &'static str, r: Result<T>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            components.insert(name, format!("unavailable: {e}"));
            None
        }
    }
}

fn need(path: &Option<PathBuf>, what: &'static str) -> Result<PathBuf> {
    path.clone().ok_or(Error::NotImplemented(what))
}

impl AppState {
    pub fn load(cfg: &ServerConfig) -> Result<AppState> {
        let snap: &'static Snapshot = Box::leak(Box::new(Snapshot::open(&cfg.snapshot)?));
        let mut comp = BTreeMap::new();
        let fts = optional(&mut comp, "registry", need(&cfg.db, "no --db given").and_then(|db| registry::probe(&db)));
        let db = fts.map(|_| cfg.db.clone().expect("probed"));
        if let Some(has_fts) = fts {
            comp.insert("registry", format!("ok: {}", cfg.db.as_ref().expect("probed").display()));
            if !has_fts {
                comp.insert("registry_fts", "empty: run Store.rebuild_fts() for the lexical fallback".into());
            }
        }
        let search = optional(
            &mut comp,
            "search",
            need(&cfg.index_dir, "no --index given").and_then(|d| atlas_search::SearchIndex::open(snap, &d)),
        );
        let lane = match (&search, &db) {
            (Some(_), _) => "ok: atlas-search".to_string(),
            (None, Some(_)) => "fallback: registry FTS5 lexical lane (Store.search semantics)".to_string(),
            (None, None) => comp.get("search").cloned().unwrap_or_default(),
        };
        comp.insert("search", lane);
        let paths = optional(
            &mut comp,
            "paths",
            need(&cfg.composition, "no --composition given").and_then(|p| atlas_paths::CompositionTable::load(&p)),
        )
        .map(|t| atlas_paths::PathEngine::new(snap, t));
        if paths.is_some() {
            comp.insert("paths", "ok".into());
        }
        let layout = optional(
            &mut comp,
            "layout",
            need(&cfg.layout_dir, "no --layout given").and_then(|d| atlas_layout::Layout::open(snap, &d)),
        );
        if let Some(l) = &layout {
            comp.insert("layout", format!("ok: {}", l.version));
        }
        let mut seed = blake3::Hasher::new();
        seed.update(snap.content_hash_hex().as_bytes());
        seed.update(layout.as_ref().map_or("", |l| l.version.as_str()).as_bytes());
        let started_at = now_utc();
        let permits = cfg.permits();
        let cors_origin = match &cfg.cors_origin {
            Some(o) => Some(HeaderValue::from_str(o).map_err(|e| Error::BadRequest(format!("cors origin: {e}")))?),
            None => None,
        };
        let engine = Engine {
            coverage: coverage::compute(snap, &started_at),
            snap,
            db,
            search,
            paths,
            layout,
            components: comp,
            started_at,
            etag_seed: *seed.finalize().as_bytes(),
            heavy: Arc::new(Semaphore::new(permits)),
        };
        Ok(AppState { engine: Arc::new(engine), web_dir: cfg.web_dir.clone(), cors_origin })
    }
}

/// Every route of SPEC §15, `/api/health`, and the web client as the fallback.
pub fn router(state: AppState) -> Router {
    let cached = Router::new()
        .route("/api/neighborhood", get(graph::handler))
        .route("/api/path", get(paths::handler))
        .route("/api/map/tiles", get(tiles::by_query))
        .route("/api/map/tiles/{z}/{x}/{y}", get(tiles::by_path))
        .route("/api/map/locate", get(tiles::locate))
        .route("/api/coverage", get(coverage::coverage))
        .route_layer(from_fn_with_state(state.clone(), http::etag));
    let web = ServeDir::new(&state.web_dir)
        .precompressed_br()
        .precompressed_gzip()
        .append_index_html_on_directories(true);
    let cors = state.cors_origin.clone().map(|o| {
        CorsLayer::new().allow_origin(o).allow_methods([Method::GET, Method::HEAD]).max_age(Duration::from_secs(600))
    });
    Router::new()
        .route("/api/health", get(coverage::health))
        .route("/api/search", get(search::handler))
        .route("/api/entity/{id}", get(entity::handler))
        .route("/api/concept", get(concept::handler))
        .route("/api/concept/edges", get(concept::edges))
        .merge(cached)
        .route("/api/{*rest}", get(http::api_not_found))
        .fallback_service(web)
        .with_state(state)
        .layer(from_fn(http::headers))
        .layer(tower::util::option_layer(cors))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(TimeoutLayer::with_status_code(axum::http::StatusCode::REQUEST_TIMEOUT, REQUEST_TIMEOUT))
        .layer(CompressionLayer::new().gzip(true).br(true).quality(CompressionLevel::Precise(COMPRESSION_QUALITY)))
}

/// Serve `router(state)` on `addr` until Ctrl-C.
pub async fn serve(addr: SocketAddr, state: AppState) -> Result<()> {
    // TCP_NODELAY: without it Nagle + delayed ACK holds small responses ~10-40 ms
    // on loopback (measured: /api/health p50 13.6 ms with 16 keep-alive connections).
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| Error::io(addr.to_string(), e))?
        .tap_io(|tcp| {
            let _ = tcp.set_nodelay(true);
        });
    eprintln!("atlas: serving http://{addr} (snapshot {})", state.engine.snap.content_hash_hex());
    for (k, v) in &state.engine.components {
        eprintln!("atlas:   {k}: {v}");
    }
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| Error::io(addr.to_string(), e))
}
