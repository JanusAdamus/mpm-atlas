//! `atlas`: command-line entry point of the Rust engine.
//!
//! atlas compile [--db data/atlas.db] [--out data/snapshot] [--threads N] [--json]
//! atlas stats   [--snapshot data/snapshot] [--verify] [--json]
//! atlas serve   [--snapshot …] [--db …] [--web web/dist] [--index …] [--layout …] [--composition …] [--addr 127.0.0.1:8765]
//! atlas index  [--snapshot …] [--db …] [--out data/index]   (atlas-search indexes)
//! atlas search | path | layout   (call their crates; search falls back to the registry FTS5 lane)
//!
//! With `--json`, stdout carries exactly one JSON value; errors are the SPEC §15
//! envelope `{"error": {"code", "message"}}` and the exit code is 1.

use atlas_core::{CompileOptions, Error, Result, Snapshot};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "atlas", version, about = "Atlas engine: snapshot, search, paths, layout, server")]
struct Cli {
    /// Print one JSON value on stdout (machine-readable; used by bench/ and atlas/cli.py).
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Compile the SQLite registry into a memory-mapped snapshot.
    Compile {
        #[arg(long, default_value = "data/atlas.db")]
        db: PathBuf,
        #[arg(long, default_value = "data/snapshot")]
        out: PathBuf,
        /// Worker threads (0 = all cores).
        #[arg(long, default_value_t = 0)]
        threads: usize,
        /// Also build the derived artifacts: search indexes (--index) and map layout + tiles (--layout).
        #[arg(long)]
        all: bool,
        #[arg(long, default_value = "data/index")]
        index: PathBuf,
        #[arg(long, default_value = "data/layout")]
        layout: PathBuf,
    },
    /// Counts of a snapshot; --verify recomputes the content hash and checks references.
    Stats {
        #[arg(long, default_value = "data/snapshot")]
        snapshot: PathBuf,
        #[arg(long)]
        verify: bool,
    },
    /// Serve the JSON API and the web client.
    Serve {
        #[arg(long, default_value = "data/snapshot")]
        snapshot: PathBuf,
        #[arg(long, default_value = "data/atlas.db")]
        db: PathBuf,
        #[arg(long, default_value = "web/dist")]
        web: PathBuf,
        #[arg(long, default_value = "127.0.0.1:8765")]
        addr: std::net::SocketAddr,
        #[arg(long, default_value = "data/index")]
        index: PathBuf,
        #[arg(long, default_value = "data/layout")]
        layout: PathBuf,
        #[arg(long, default_value = "shapes/composition.json")]
        composition: PathBuf,
        /// Allow this origin cross-site (CORS is off by default).
        #[arg(long)]
        cors_origin: Option<String>,
        /// Concurrent heavy requests before shedding with 503 (0 = 2 x cores).
        #[arg(long, default_value_t = 0)]
        heavy_permits: usize,
    },
    /// Build the atlas-search indexes for a snapshot (text read from the registry).
    Index {
        #[arg(long, default_value = "data/snapshot")]
        snapshot: PathBuf,
        #[arg(long, default_value = "data/atlas.db")]
        db: PathBuf,
        #[arg(long, default_value = "data/index")]
        out: PathBuf,
    },
    /// Hybrid search (same response as GET /api/search).
    Search {
        query: String,
        #[arg(long, default_value = "es")]
        lang: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Comma-separated entity types.
        #[arg(long, value_delimiter = ',')]
        types: Vec<String>,
        #[arg(long, default_value = "data/snapshot")]
        snapshot: PathBuf,
        #[arg(long, default_value = "data/index")]
        index: PathBuf,
        /// Registry for the lexical fallback when the atlas-search index is absent.
        #[arg(long, default_value = "data/atlas.db")]
        db: PathBuf,
    },
    /// Typed paths between two entities (same response as GET /api/path).
    Path {
        from: String,
        to: String,
        #[arg(long, default_value = "best_supported")]
        objective: String,
        #[arg(long, default_value_t = 6)]
        max_hops: u32,
        #[arg(long, default_value = "data/snapshot")]
        snapshot: PathBuf,
        #[arg(long, default_value = "shapes/composition.json")]
        composition: PathBuf,
    },
    /// Communities, layout and tiles for the map (warm-started from the layout in --out).
    Layout {
        #[arg(long, default_value = "data/snapshot")]
        snapshot: PathBuf,
        #[arg(long, default_value = "data/layout")]
        out: PathBuf,
    },
}

fn run(cmd: Cmd) -> Result<Value> {
    match cmd {
        Cmd::Compile { db, out, threads, all, index, layout } => {
            let mut opts = CompileOptions::new(db.clone(), out.clone());
            opts.threads = threads;
            let report = serde_json::to_value(atlas_core::compile(&opts)?).expect("report serializes");
            if !all {
                return Ok(report);
            }
            // Derived artifacts, each keyed by (or warm-started from) the new snapshot.
            let snap = Snapshot::open(&out)?;
            let t = Instant::now();
            let idx = atlas_search::SearchIndex::build(&snap, &db, &index)?;
            let index_s = t.elapsed().as_secs_f64();
            let t = Instant::now();
            let lay = atlas_layout::build(&snap, &atlas_layout::LayoutParams::default(), &layout)?;
            let layout_s = t.elapsed().as_secs_f64();
            Ok(json!({
                "snapshot": report,
                "index": idx, "index_s": index_s,
                "layout": lay, "layout_s": layout_s,
            }))
        }
        Cmd::Stats { snapshot, verify } => {
            let t = Instant::now();
            let snap = Snapshot::open(&snapshot)?;
            let open_ms = t.elapsed().as_secs_f64() * 1e3;
            let verify_ms = if verify {
                let t = Instant::now();
                snap.verify()?;
                Some(t.elapsed().as_secs_f64() * 1e3)
            } else {
                None
            };
            let mut v = serde_json::to_value(snap.stats()).expect("stats serialize");
            v["open_ms"] = json!(open_ms);
            v["verified"] = json!(verify);
            v["verify_ms"] = json!(verify_ms);
            Ok(v)
        }
        Cmd::Serve { snapshot, db, web, addr, index, layout, composition, cors_origin, heavy_permits } => {
            let cfg = atlas_server::ServerConfig {
                snapshot,
                db: Some(db),
                web_dir: web,
                index_dir: Some(index),
                layout_dir: Some(layout),
                composition: Some(composition),
                cors_origin,
                heavy_permits,
            };
            let state = atlas_server::AppState::load(&cfg)?;
            let rt = cfg.runtime()?;
            rt.block_on(atlas_server::serve(addr, state))?;
            Ok(json!({"served": addr.to_string()}))
        }
        Cmd::Index { snapshot, db, out } => {
            let snap = Snapshot::open(&snapshot)?;
            let report = atlas_search::SearchIndex::build(&snap, &db, &out)?;
            Ok(serde_json::to_value(report).expect("report serializes"))
        }
        Cmd::Search { query, lang, limit, types, snapshot, index, db } => {
            let snap = Snapshot::open(&snapshot)?;
            let types = types
                .iter()
                .map(|t| t.parse().map_err(|e: atlas_core::vocab::UnknownVariant| Error::BadRequest(e.to_string())))
                .collect::<Result<_>>()?;
            let q = atlas_search::SearchQuery { q: query, lang, types, limit };
            let resp = match atlas_search::SearchIndex::open(&snap, &index) {
                Ok(idx) => idx.search(&snap, &q)?,
                Err(_) if db.exists() => atlas_server::registry_search(&snap, &db, &q)?,
                Err(e) => return Err(e),
            };
            Ok(serde_json::to_value(resp).expect("response serializes"))
        }
        Cmd::Path { from, to, objective, max_hops, snapshot, composition } => {
            let snap = Snapshot::open(&snapshot)?;
            let objective = serde_json::from_value(json!(objective))
                .map_err(|e| Error::BadRequest(format!("objective: {e}")))?;
            let table = atlas_paths::CompositionTable::load(&composition)?;
            let engine = atlas_paths::PathEngine::new(&snap, table);
            let q = atlas_paths::PathQuery {
                from,
                to,
                objective,
                max_hops,
                include_disputed: false,
                lang: "es".into(),
                weights: Default::default(),
            };
            Ok(serde_json::to_value(engine.find(&q)?).expect("response serializes"))
        }
        Cmd::Layout { snapshot, out } => {
            let snap = Snapshot::open(&snapshot)?;
            // Warm-starts from the current layout in `out`, so existing nodes barely move.
            let report = atlas_layout::build(&snap, &atlas_layout::LayoutParams::default(), &out)?;
            Ok(serde_json::to_value(report).expect("report serializes"))
        }
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli.cmd) {
        Ok(v) => {
            if cli.json {
                println!("{v}");
            } else {
                println!("{}", serde_json::to_string_pretty(&v).expect("value serializes"));
            }
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            if cli.json {
                println!("{}", e.to_json());
            } else {
                eprintln!("atlas: {e}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}
