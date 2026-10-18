//! Snapshot benchmark at realistic scale (synthetic registry).
//!
//! cargo run --release -p atlas-core --example bench_snapshot -- \
//!     --dir C:/Users/AdrianGonzalez/.cargo-target/bench-data --entities 1000000 --edges 10000000
//!
//! 1. creates `<dir>/synth.db` if missing: schema through the Python Store (the
//!    contract), rows through `atlas_core::synth` (timed separately);
//! 2. compiles it `--reps` times (wall time and phases; peak RSS sampled);
//! 3. opens the snapshot `--reps` times (warm page cache) and verifies the hash;
//! 4. measures neighbor iteration: full CSR scans (1 thread and all threads),
//!    random-node typed neighbor lists, 2-hop expansions, and id lookups.
//!
//! Prints one JSON object on stdout.

use atlas_core::synth::{SynthConfig, generate};
use atlas_core::{CompileOptions, Dir, RelationType, Snapshot, compile};
use rayon::prelude::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Args {
    dir: PathBuf,
    entities: u32,
    edges: u64,
    reps: usize,
    threads: usize,
}

fn args() -> Args {
    let mut a = Args { dir: PathBuf::from("bench-data"), entities: 1_000_000, edges: 10_000_000, reps: 3, threads: 0 };
    let v: Vec<String> = std::env::args().skip(1).collect();
    for p in v.chunks(2) {
        let val = p.get(1).expect("flag needs a value");
        match p[0].as_str() {
            "--dir" => a.dir = val.into(),
            "--entities" => a.entities = val.parse().unwrap(),
            "--edges" => a.edges = val.parse().unwrap(),
            "--reps" => a.reps = val.parse().unwrap(),
            "--threads" => a.threads = val.parse().unwrap(),
            f => panic!("unknown flag {f}"),
        }
    }
    a
}

/// Peak memory of this process, sampled every 20 ms: working set (includes mapped
/// file pages of the registry and snapshot) and private bytes (heap; Windows PagefileUsage).
struct PeakRss {
    peak: Arc<AtomicU64>,
    private: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl PeakRss {
    fn start() -> Self {
        let (peak, stop) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicBool::new(false)));
        let private = Arc::new(AtomicU64::new(0));
        let (p, v, s) = (peak.clone(), private.clone(), stop.clone());
        std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                if let Some(m) = memory_stats::memory_stats() {
                    p.fetch_max(m.physical_mem as u64, Ordering::Relaxed);
                    v.fetch_max(m.virtual_mem as u64, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        PeakRss { peak, private, stop }
    }
    /// (peak working set MiB, peak private MiB)
    fn mib(self) -> serde_json::Value {
        self.stop.store(true, Ordering::Relaxed);
        let m = |a: &AtomicU64| a.load(Ordering::Relaxed) as f64 / (1 << 20) as f64;
        json!({"working_set": m(&self.peak), "private": m(&self.private)})
    }
}

fn pct(v: &mut [f64], q: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * q).round() as usize]
}

fn ensure_db(a: &Args) -> serde_json::Value {
    std::fs::create_dir_all(&a.dir).unwrap();
    let db = a.dir.join("synth.db");
    if db.exists() {
        return json!({"reused": db.display().to_string()});
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let py = std::env::var("ATLAS_PYTHON").unwrap_or_else(|_| "python".into());
    let ok = Command::new(py)
        .current_dir(&root)
        .args(["-c", "import sys; from atlas.store import Store; Store(sys.argv[1]).close()"])
        .arg(&db)
        .status()
        .expect("run python to create the registry schema");
    assert!(ok.success(), "schema creation failed");
    let rss = PeakRss::start();
    let r = generate(&db, &SynthConfig { entities: a.entities, edges: a.edges, seed: 42 }).expect("synth");
    let db_bytes = std::fs::metadata(&db).unwrap().len();
    json!({"report": r, "db_bytes": db_bytes, "peak_mem_mib": rss.mib()})
}

fn bench_compile(a: &Args) -> (serde_json::Value, PathBuf) {
    let out = a.dir.join("snapshot");
    let mut opts = CompileOptions::new(a.dir.join("synth.db"), &out);
    opts.threads = a.threads;
    let mut runs = Vec::new();
    for _ in 0..a.reps {
        let rss = PeakRss::start();
        let r = compile(&opts).expect("compile");
        runs.push(json!({"total_secs": r.total_secs, "phases": r.phases, "peak_mem_mib": rss.mib(), "report": r}));
    }
    (json!(runs), out)
}

fn bench_load(a: &Args, dir: &Path) -> serde_json::Value {
    let mut open_ms: Vec<f64> = (0..a.reps.max(5) * 4)
        .map(|_| {
            let t = Instant::now();
            let s = Snapshot::open(dir).unwrap();
            std::hint::black_box(s.num_edges());
            t.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    let s = Snapshot::open(dir).unwrap();
    let t = Instant::now();
    s.verify_hash().unwrap();
    let hash_ms = t.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    s.verify().unwrap();
    let verify_ms = t.elapsed().as_secs_f64() * 1e3;
    json!({"open_ms_p50": pct(&mut open_ms, 0.5), "open_ms_p95": pct(&mut open_ms, 0.95),
           "verify_hash_ms": hash_ms, "verify_full_ms": verify_ms, "file_bytes": s.file_len()})
}

fn full_scan(s: &Snapshot, dir: Dir, parallel: bool) -> (u64, f64) {
    let csr = s.csr(dir);
    let n = s.num_nodes();
    let body = |v: u32| -> u64 {
        let e = csr.edges(v);
        let mut acc = 0u64;
        for i in 0..e.len() {
            acc = acc.wrapping_add(e.nbr[i] as u64 ^ e.claim[i] as u64) + e.rel[i] as u64 + e.state[i] as u64;
        }
        acc
    };
    let t = Instant::now();
    let acc = if parallel { (0..n).into_par_iter().map(body).sum::<u64>() } else { (0..n).map(body).sum() };
    std::hint::black_box(acc);
    (s.num_edges() as u64, t.elapsed().as_secs_f64())
}

fn bench_iter(s: &Snapshot) -> serde_json::Value {
    let n = s.num_nodes();
    let mut res = serde_json::Map::new();
    for (name, dir, par) in [("scan_out_1t", Dir::Out, false), ("scan_out_par", Dir::Out, true), ("scan_in_par", Dir::In, true)] {
        let mut best = f64::MAX;
        for _ in 0..5 {
            best = best.min(full_scan(s, dir, par).1);
        }
        res.insert(name.into(), json!({"edges": s.num_edges(), "best_secs": best,
                                        "edges_per_sec": s.num_edges() as f64 / best}));
    }
    // random nodes: typed neighbor list (depends_on_logically, both directions)
    let mut x = 0x2545F4914F6CDD1Du64;
    let mut rnd = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        (x % n as u64) as u32
    };
    let (out, inn) = (s.csr(Dir::Out), s.csr(Dir::In));
    let q = 1_000_000;
    let t = Instant::now();
    let mut touched = 0u64;
    for _ in 0..q {
        let v = rnd();
        touched += out.edges_rel(v, RelationType::DependsOnLogically).len() as u64;
        touched += inn.edges_rel(v, RelationType::DependsOnLogically).len() as u64;
    }
    let secs = t.elapsed().as_secs_f64();
    res.insert("random_typed_1t".into(), json!({"queries": q, "edges": touched, "ns_per_query": secs * 1e9 / q as f64}));
    // 2-hop expansion over all relations, out+in, from random seeds
    let mut lat = Vec::new();
    let mut sizes = Vec::new();
    let mut seen = vec![0u32; n as usize];
    for stamp in 1..=2000u32 {
        let seed = rnd();
        let t = Instant::now();
        let mut frontier = vec![seed];
        seen[seed as usize] = stamp;
        let mut reached = 1u64;
        for _ in 0..2 {
            let mut next = Vec::new();
            for &v in &frontier {
                for csr in [&out, &inn] {
                    for &w in csr.edges(v).nbr {
                        if seen[w as usize] != stamp {
                            seen[w as usize] = stamp;
                            next.push(w);
                        }
                    }
                }
            }
            reached += next.len() as u64;
            frontier = next;
        }
        lat.push(t.elapsed().as_secs_f64() * 1e6);
        sizes.push(reached as f64);
    }
    res.insert("two_hop_1t".into(), json!({"seeds": 2000, "us_p50": pct(&mut lat.clone(), 0.5),
        "us_p95": pct(&mut lat, 0.95), "reached_p50": pct(&mut sizes.clone(), 0.5), "reached_p95": pct(&mut sizes, 0.95)}));
    // id lookups (binary search over the mmap'd sorted permutation)
    let ids: Vec<String> = (0..100_000).map(|_| s.node_id(rnd()).to_owned()).collect();
    let t = Instant::now();
    let found = ids.iter().filter(|id| s.find_node(id).is_some()).count();
    let secs = t.elapsed().as_secs_f64();
    assert_eq!(found, ids.len());
    res.insert("find_node_1t".into(), json!({"lookups": ids.len(), "ns_per_lookup": secs * 1e9 / ids.len() as f64}));
    serde_json::Value::Object(res)
}

fn main() {
    let a = args();
    let synth = ensure_db(&a);
    let (compile_runs, dir) = bench_compile(&a);
    let load = bench_load(&a, &dir);
    let s = Snapshot::open(&dir).unwrap();
    let iter = bench_iter(&s);
    let out = json!({
        "machine": {"threads": rayon::current_num_threads(), "os": std::env::consts::OS},
        "args": {"entities": a.entities, "edges": a.edges, "reps": a.reps},
        "synth": synth, "compile": compile_runs, "load": load, "iteration": iter, "stats": s.stats(),
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
