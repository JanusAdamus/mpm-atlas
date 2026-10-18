//! Layout benchmark on a compiled snapshot.
//!
//! cargo run --release -p atlas-layout --example bench_layout -- \
//!     --snapshot C:/Users/AdrianGonzalez/.cargo-target/bench-1m/snapshot \
//!     --out C:/Users/AdrianGonzalez/.cargo-target/layout-bench/out --drop 0.01
//!
//! 1. cold build (graph, Leiden per level, multilevel layout, tiles) with peak memory;
//! 2. write, reopen, and time `tile()` (JSON included) on random stored tiles;
//! 3. stability: a build without a random `--drop` share of nodes, then a warm
//!    rebuild with them added; displacement of the common nodes, compared with
//!    a cold rebuild (the full build of step 1);
//! 4. `--det 1`: a second cold layout must give bit-identical positions.
//!
//! `--db <registry.db>` first compiles that registry into `--snapshot` (real data).
//!
//! Prints one JSON object on stdout.

use atlas_core::{Projection, Snapshot};
use atlas_layout::{ComputeOptions, Layout, LayoutParams, Prior, compute_with, tile};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Args {
    db: Option<PathBuf>,
    snapshot: PathBuf,
    out: PathBuf,
    drop: f64,
    det: bool,
    tile_samples: usize,
    resolutions: Option<Vec<f64>>,
    gravity: Option<f64>,
    strong_gravity: Option<bool>,
    stability: bool,
}

fn args() -> Args {
    let mut a = Args {
        db: None,
        snapshot: "data/snapshot".into(),
        out: "layout-bench".into(),
        drop: 0.01,
        det: false,
        tile_samples: 2000,
        resolutions: None,
        gravity: None,
        strong_gravity: None,
        stability: true,
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    for p in v.chunks(2) {
        let val = p.get(1).expect("flag needs a value");
        match p[0].as_str() {
            "--db" => a.db = Some(val.into()),
            "--snapshot" => a.snapshot = val.into(),
            "--out" => a.out = val.into(),
            "--drop" => a.drop = val.parse().unwrap(),
            "--det" => a.det = val == "1",
            "--tile-samples" => a.tile_samples = val.parse().unwrap(),
            "--resolutions" => {
                a.resolutions = Some(val.split(',').map(|x| x.parse().unwrap()).collect())
            }
            "--gravity" => a.gravity = Some(val.parse().unwrap()),
            "--strong-gravity" => a.strong_gravity = Some(val == "1"),
            "--stability" => a.stability = val == "1",
            f => panic!("unknown flag {f}"),
        }
    }
    a
}

/// Peak working set and private bytes, sampled every 20 ms.
struct Peak {
    ws: Arc<AtomicU64>,
    private: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Peak {
    fn start() -> Peak {
        let p = Peak {
            ws: Arc::default(),
            private: Arc::default(),
            stop: Arc::default(),
        };
        let (ws, pr, stop) = (p.ws.clone(), p.private.clone(), p.stop.clone());
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Some(m) = memory_stats::memory_stats() {
                    ws.fetch_max(m.physical_mem as u64, Ordering::Relaxed);
                    pr.fetch_max(m.virtual_mem as u64, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        p
    }
    fn finish(self) -> Value {
        self.stop.store(true, Ordering::Relaxed);
        let gib = |x: u64| x as f64 / (1u64 << 30) as f64;
        json!({"peak_working_set_gib": gib(self.ws.load(Ordering::Relaxed)),
               "peak_private_gib": gib(self.private.load(Ordering::Relaxed))})
    }
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(f64::total_cmp);
    v[((v.len() as f64 - 1.0) * p).round() as usize]
}

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn displacement(a: &Layout, b: &Layout, nodes: &[usize]) -> Value {
    let mut d: Vec<f64> = nodes
        .iter()
        .map(|&v| {
            let (p, q) = (a.positions[v], b.positions[v]);
            (((p[0] - q[0]) as f64).powi(2) + ((p[1] - q[1]) as f64).powi(2)).sqrt()
        })
        .collect();
    let mean = d.iter().sum::<f64>() / d.len() as f64;
    json!({"nodes": d.len(), "mean": mean, "p50": pct(&mut d, 0.5), "p95": pct(&mut d, 0.95),
           "p99": pct(&mut d, 0.99), "max": pct(&mut d, 1.0)})
}

/// Where the nodes sit in the unit square: quantiles of x and y for nodes with
/// relations and for "unknown" ones, and how many of the 256 zoom-4 cells hold any.
fn extent(l: &Layout) -> Value {
    let last = l.levels.len() - 1;
    let unknown = l.communities[last].unknown;
    let (mut rel, mut unk): (Vec<[f32; 2]>, Vec<[f32; 2]>) = (Vec::new(), Vec::new());
    for (v, p) in l.positions.iter().enumerate() {
        if p[0].is_finite() {
            if Some(l.levels[last][v]) == unknown {
                unk.push(*p)
            } else {
                rel.push(*p)
            }
        }
    }
    let q = |pts: &[[f32; 2]]| -> Value {
        if pts.is_empty() {
            return Value::Null;
        }
        let mut xs: Vec<f64> = pts.iter().map(|p| p[0] as f64).collect();
        let mut ys: Vec<f64> = pts.iter().map(|p| p[1] as f64).collect();
        json!({"nodes": pts.len(),
               "x": [pct(&mut xs, 0.01), pct(&mut xs, 0.5), pct(&mut xs, 0.99)],
               "y": [pct(&mut ys, 0.01), pct(&mut ys, 0.5), pct(&mut ys, 0.99)]})
    };
    let mut cells: Vec<u32> = rel
        .iter()
        .chain(&unk)
        .map(|p| ((p[0] * 16.0).min(15.0) as u32) << 4 | (p[1] * 16.0).min(15.0) as u32)
        .collect();
    cells.sort_unstable();
    cells.dedup();
    json!({"related": q(&rel), "unknown": q(&unk), "z4_cells_used": cells.len()})
}

fn main() {
    let a = args();
    let compile_report = a.db.as_ref().map(|db| {
        let r = atlas_core::compile(&atlas_core::CompileOptions::new(db, &a.snapshot))
            .expect("compile registry");
        serde_json::to_value(&r).unwrap()
    });
    let t = Instant::now();
    let snap = Snapshot::open(&a.snapshot).expect("open snapshot");
    let open_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut params = LayoutParams::default();
    if let Some(r) = a.resolutions.clone() {
        params.resolutions = r;
    }
    if let Some(g) = a.gravity {
        params.force.gravity = g;
    }
    if let Some(s) = a.strong_gravity {
        params.force.strong_gravity = s;
    }
    let mut out = json!({
        "snapshot": a.snapshot, "snapshot_hash": snap.content_hash_hex(), "nodes": snap.num_nodes(),
        "edges": snap.num_edges(), "threads": rayon::current_num_threads(), "open_ms": open_ms,
        "params": format!("{params:?}"), "compile": compile_report,
    });

    // 1. cold build
    let peak = Peak::start();
    let t = Instant::now();
    let full = compute_with(&snap, &params, &ComputeOptions::default()).expect("compute");
    out["cold_secs"] = json!(t.elapsed().as_secs_f64());
    out["cold_memory"] = peak.finish();
    out["communities"] = serde_json::to_value(
        full.communities
            .iter()
            .map(|c| serde_json::to_value(c).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap();
    out["stats"] = serde_json::to_value(&full.stats).unwrap();
    out["tiles"] = serde_json::to_value(&full.tiles.meta).unwrap();
    out["tile_data_bytes"] = json!(full.tiles.data_bytes());
    out["extent"] = extent(&full);
    eprintln!(
        "cold build done in {:.2} s: {}",
        out["cold_secs"].as_f64().unwrap(),
        out["stats"]
    );

    // 2. write, reopen, query
    let t = Instant::now();
    let rep = full.write(&a.out).expect("write");
    out["write_secs"] = json!(t.elapsed().as_secs_f64());
    out["files"] = json!({"layout_bytes": rep.layout_bytes, "tiles_bytes": rep.tiles_bytes, "tiles": rep.tiles});
    let t = Instant::now();
    let opened = Layout::open(&snap, &a.out).expect("open layout");
    out["open_layout_ms"] = json!(t.elapsed().as_secs_f64() * 1e3);
    let keys: Vec<(u32, u32, u32)> = opened.tiles.keys().collect();
    let mut lat = Vec::new();
    let mut sizes = Vec::new();
    for i in 0..a.tile_samples.min(keys.len() * 4) {
        let (z, x, y) = keys[(mix(i as u64) % keys.len() as u64) as usize];
        let layer = Projection::ALL[i % Projection::ALL.len()];
        let t = Instant::now();
        let r = tile(&snap, &opened, layer, z, x, y, "es").expect("tile");
        let body = serde_json::to_vec(&r).unwrap();
        lat.push(t.elapsed().as_secs_f64() * 1e3);
        sizes.push(body.len() as f64);
    }
    if !lat.is_empty() {
        out["tile_query_ms"] = json!({"samples": lat.len(), "p50": pct(&mut lat, 0.5), "p95": pct(&mut lat, 0.95),
                                      "max": pct(&mut lat, 1.0)});
        out["tile_json_bytes"] = json!({"p50": pct(&mut sizes, 0.5), "p95": pct(&mut sizes, 0.95), "max": pct(&mut sizes, 1.0)});
    }
    drop(opened);
    eprintln!("tiles queried");

    // 3. stability with `drop` share of nodes added
    if a.stability {
        let n = snap.num_nodes() as usize;
        let threshold = (a.drop * u64::MAX as f64) as u64;
        let exclude: Vec<bool> = (0..n)
            .map(|v| !snap.is_merged(v as u32) && mix(v as u64 ^ 0xD80B) < threshold)
            .collect();
        let dropped = exclude.iter().filter(|&&e| e).count();
        let opts = ComputeOptions {
            exclude: Some(exclude.clone()),
            skip_tiles: true,
            ..Default::default()
        };
        let t = Instant::now();
        let old = compute_with(&snap, &params, &opts).expect("old build");
        let old_secs = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let prior = Prior::from_layout(&old);
        let warm = compute_with(
            &snap,
            &params,
            &ComputeOptions {
                prior: Some(prior),
                skip_tiles: true,
                ..Default::default()
            },
        )
        .expect("warm build");
        let warm_secs = t.elapsed().as_secs_f64();
        let common: Vec<usize> = (0..n)
            .filter(|&v| !exclude[v] && !snap.is_merged(v as u32))
            .collect();
        // Nodes that did not cross the known / unknown divide (a node whose only
        // neighbours were the dropped ones is unknown in `old` and moves into the map).
        let unk = |l: &Layout, v: usize| {
            let last = l.levels.len() - 1;
            Some(l.levels[last][v]) == l.communities[last].unknown
        };
        let same: Vec<usize> = common
            .iter()
            .copied()
            .filter(|&v| unk(&old, v) == unk(&warm, v))
            .collect();
        out["stability"] = json!({
            "added_nodes": dropped,
            "old_build_secs": old_secs,
            "warm_build_secs": warm_secs,
            "warm_layout_secs": warm.stats.secs_layout,
            "warm": warm.stats.warm,
            "warm_runs": warm.stats.runs,
            "displacement_warm": displacement(&old, &warm, &common),
        "status_changed": common.len() - same.len(),
        "displacement_warm_same_status": displacement(&old, &warm, &same),
            "displacement_cold": displacement(&old, &full, &common),
        });
        eprintln!("stability done");
    }

    // 4. determinism
    if a.det {
        let again = compute_with(
            &snap,
            &params,
            &ComputeOptions {
                skip_tiles: true,
                ..Default::default()
            },
        )
        .expect("again");
        out["deterministic_positions"] =
            json!(again.positions == full.positions && again.levels == full.levels);
    }
    println!("{out}");
}
