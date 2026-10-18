//! Path engine benchmark: latency per objective and query class, memory, throughput.
//!
//!     cargo run --release -p atlas-paths --example bench -- \
//!         --snapshot <dir|file> [--composition shapes/composition.json] [--queries 200] \
//!         [--k 3] [--landmarks 16] [--max-hops 6] [--seed 1] [--compile-db <atlas.db>] [--concepts 0|1]
//!
//! Query classes (reported apart, never pooled):
//! - `reachable`: t is the end of a random authorized walk of 2..=5 steps from s, so
//!   an authorized route is known to exist;
//! - `random`: s and t uniform among nodes with claims (mostly abstentions).
//!
//! Latency is wall clock around `PathEngine::find_with`, one query at a time.

use atlas_core::{CompileOptions, Dir, EvidenceState, NodeIx, Snapshot, compile};
use atlas_paths::algebra::{EMPTY, NONE, Way};
use atlas_paths::{CompositionTable, EngineOptions, Objective, PathEngine, PathOptions, PathQuery};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

struct Args {
    snapshot: PathBuf,
    composition: PathBuf,
    queries: usize,
    k: usize,
    landmarks: usize,
    max_hops: u32,
    seed: u64,
    compile_db: Option<PathBuf>,
    derive_budget: usize,
    alt_factor: f64,
    alt_min: usize,
    concepts: bool,
}

fn args() -> Args {
    let mut a = Args {
        snapshot: PathBuf::new(),
        composition: PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../shapes/composition.json")),
        queries: 200,
        k: 3,
        landmarks: 16,
        max_hops: 6,
        seed: 1,
        compile_db: None,
        derive_budget: PathOptions::default().derive_budget,
        alt_factor: PathOptions::default().alt_budget_factor,
        alt_min: PathOptions::default().alt_min_expansions,
        concepts: true,
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    for p in v.chunks(2) {
        let val = p.get(1).cloned().unwrap_or_default();
        match p[0].as_str() {
            "--snapshot" => a.snapshot = val.into(),
            "--composition" => a.composition = val.into(),
            "--queries" => a.queries = val.parse().expect("--queries"),
            "--k" => a.k = val.parse().expect("--k"),
            "--landmarks" => a.landmarks = val.parse().expect("--landmarks"),
            "--max-hops" => a.max_hops = val.parse().expect("--max-hops"),
            "--seed" => a.seed = val.parse().expect("--seed"),
            "--compile-db" => a.compile_db = Some(val.into()),
            "--derive-budget" => a.derive_budget = val.parse().expect("--derive-budget"),
            "--alt-factor" => a.alt_factor = val.parse().expect("--alt-factor"),
            "--alt-min" => a.alt_min = val.parse().expect("--alt-min"),
            "--concepts" => a.concepts = val != "0",
            x => panic!("unknown argument {x}"),
        }
    }
    assert!(!a.snapshot.as_os_str().is_empty(), "--snapshot is required");
    a
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn mem() -> (u64, u64) {
    memory_stats::memory_stats().map_or((0, 0), |m| (m.physical_mem as u64, m.virtual_mem as u64))
}

fn mib(b: u64) -> f64 {
    (b as f64 / 1048576.0 * 10.0).round() / 10.0
}

fn pick_node(snap: &Snapshot, rng: &mut Rng) -> NodeIx {
    loop {
        let n = rng.below(snap.num_nodes() as u64) as NodeIx;
        if !snap.evidence(n).is_unknown() && !snap.is_merged(n) {
            return n;
        }
    }
}

/// End of a random authorized walk of `len` steps (current, undisputed claims).
fn walk(snap: &Snapshot, table: &CompositionTable, rng: &mut Rng, len: usize) -> Option<(NodeIx, NodeIx)> {
    let s = pick_node(snap, rng);
    let mask = atlas_core::CURRENT_STATES_MASK & !EvidenceState::Disputed.bit();
    let (out, inn) = (snap.csr(Dir::Out), snap.csr(Dir::In));
    let (mut x, mut kind, mut seen) = (s, EMPTY, vec![s]);
    for _ in 0..len {
        let (o, i) = (out.edges(x), inn.edges(x));
        let total = o.len() + i.len();
        if total == 0 {
            return None;
        }
        let mut moved = false;
        for _ in 0..8 {
            let r = rng.below(total as u64) as usize;
            let (e, way) = if r < o.len() { (o.get(r), Way::Forward) } else { (i.get(r - o.len()), Way::Converse) };
            let Some(spec) = table.step(e.relation, way) else { continue };
            let k = table.compose(kind, spec.kind);
            if mask & e.state.bit() == 0 || k == NONE || table.strength(k) < 1 || seen.contains(&e.nbr) {
                continue;
            }
            kind = k;
            x = e.nbr;
            seen.push(x);
            moved = true;
            break;
        }
        if !moved {
            return None;
        }
    }
    Some((s, x))
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let i = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    (sorted[i] * 1000.0).round() / 1000.0
}

fn query(snap: &Snapshot, s: NodeIx, t: NodeIx, o: Objective, max_hops: u32) -> PathQuery {
    PathQuery {
        from: snap.node_id(s).into(),
        to: snap.node_id(t).into(),
        objective: o,
        max_hops,
        include_disputed: false,
        lang: "es".into(),
        weights: Default::default(),
    }
}

fn run_class(engine: &PathEngine, pairs: &[(NodeIx, NodeIx)], o: Objective, a: &Args, opts: &PathOptions, peak: &mut (u64, u64)) -> Value {
    let snap = engine.snapshot;
    let mut ms = Vec::with_capacity(pairs.len());
    let mut causes: BTreeMap<String, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let (mut found, mut expanded, mut truncated, mut hops) = (0usize, Vec::new(), 0usize, Vec::new());
    let (mut proofs, mut runs, mut pms) = (Vec::new(), Vec::new(), Vec::new());
    let (mut relaxed, mut labels) = (Vec::new(), Vec::new());
    let (mut routes, mut capped) = (0usize, 0usize);
    for &(s, t) in pairs {
        let q = query(snap, s, t, o, a.max_hops);
        let t0 = Instant::now();
        let r = engine.find_with(&q, opts).expect("query runs");
        ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        expanded.push(r.stats.expanded as f64);
        relaxed.push(r.stats.relaxed as f64);
        labels.push(r.stats.labels as f64);
        if std::env::var_os("BENCH_TRACE").is_some() {
            eprintln!("  {:?} {} -> {}: {:.1} ms, {:?}", o, s, t, ms[ms.len() - 1], r.stats);
        }
        proofs.push(r.stats.premise_proofs as f64);
        runs.push(r.stats.runs as f64);
        pms.push(r.stats.premise_micros as f64 / 1000.0);
        truncated += r.stats.truncated as usize;
        routes += r.paths.len();
        capped += r.stats.alternatives_capped as usize;
        match &r.abstention {
            Some(c) => *causes.entry(c.clone()).or_default() += 1,
            None => {
                found += 1;
                hops.push(r.paths[0].steps.len() as f64);
                *kinds.entry(r.paths[0].conclusion.kind.clone()).or_default() += 1;
            }
        }
        let m = mem();
        *peak = (peak.0.max(m.0), peak.1.max(m.1));
    }
    let sorted = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v
    };
    let (ms, expanded, hops, proofs, runs) = (sorted(ms), sorted(expanded), sorted(hops), sorted(proofs), sorted(runs));
    let (pms, relaxed, labels) = (sorted(pms), sorted(relaxed), sorted(labels));
    json!({
        "queries": pairs.len(), "found": found, "abstained": causes, "first_route_conclusion": kinds,
        "ms": {"p50": pct(&ms, 0.5), "p95": pct(&ms, 0.95), "p99": pct(&ms, 0.99), "max": pct(&ms, 1.0),
               "mean": (ms.iter().sum::<f64>() / ms.len().max(1) as f64 * 1000.0).round() / 1000.0},
        "expanded": {"p50": pct(&expanded, 0.5), "p95": pct(&expanded, 0.95), "max": pct(&expanded, 1.0)},
        "relaxed": {"p50": pct(&relaxed, 0.5), "p95": pct(&relaxed, 0.95), "max": pct(&relaxed, 1.0)},
        "labels": {"p50": pct(&labels, 0.5), "p95": pct(&labels, 0.95), "max": pct(&labels, 1.0)},
        "budget_truncated": truncated, "routes_returned": routes, "alternatives_capped": capped,
        "premise_proofs": {"p50": pct(&proofs, 0.5), "p95": pct(&proofs, 0.95), "max": pct(&proofs, 1.0)},
        "premise_ms": {"p50": pct(&pms, 0.5), "p95": pct(&pms, 0.95), "max": pct(&pms, 1.0)},
        "search_runs": {"p50": pct(&runs, 0.5), "max": pct(&runs, 1.0)},
        "route_hops_p50": pct(&hops, 0.5),
    })
}

fn main() {
    let a = args();
    let mut report = serde_json::Map::new();
    report.insert("threads".into(), json!(rayon::current_num_threads()));
    if let Some(db) = &a.compile_db {
        let t0 = Instant::now();
        let r = compile(&CompileOptions::new(db, &a.snapshot)).expect("compile");
        report.insert("compile".into(), json!({"secs": t0.elapsed().as_secs_f64(), "report": serde_json::to_value(&r).unwrap()}));
    }
    let m0 = mem();
    let t0 = Instant::now();
    let snap = Snapshot::open(&a.snapshot).expect("open snapshot");
    let table = CompositionTable::load(&a.composition).expect("composition");
    let t_open = t0.elapsed().as_secs_f64();
    let t1 = Instant::now();
    let engine = PathEngine::with_options(&snap, table, EngineOptions { landmarks: a.landmarks });
    let t_index = t1.elapsed().as_secs_f64();
    let m1 = mem();
    report.insert(
        "snapshot".into(),
        json!({"path": snap.path().display().to_string(), "file_bytes": snap.file_len(), "nodes": snap.num_nodes(),
               "claims": snap.num_claims(), "edges": snap.num_edges(), "open_secs": t_open}),
    );
    report.insert(
        "engine_build".into(),
        json!({"secs": t_index, "landmarks": a.landmarks, "index_mib": mib(engine.index.memory_bytes() as u64),
               "working_set_mib_before": mib(m0.0), "working_set_mib_after": mib(m1.0),
               "private_mib_before": mib(m0.1), "private_mib_after": mib(m1.1)}),
    );

    let mut rng = Rng(a.seed);
    if let Ok(pair) = std::env::var("BENCH_PAIR") {
        let (s, t) = pair.split_once(',').expect("BENCH_PAIR=s,t");
        let (s, t): (NodeIx, NodeIx) = (s.parse().unwrap(), t.parse().unwrap());
        let o: Objective = serde_json::from_value(json!(std::env::var("BENCH_OBJECTIVE").unwrap_or("best_supported".into()))).unwrap();
        let opts = PathOptions { k: a.k, derive_budget: a.derive_budget, alt_budget_factor: a.alt_factor, alt_min_expansions: a.alt_min, concepts: a.concepts, ..Default::default() };
        let t0 = Instant::now();
        let r = engine.find_with(&query(&snap, s, t, o, a.max_hops), &opts).unwrap();
        eprintln!("{:.1} ms {:?} {:?}", t0.elapsed().as_secs_f64() * 1000.0, r.abstention, r.stats);
        for p in &r.paths {
            eprintln!("  score {:.3} {} {:?}", p.score, p.conclusion.kind, p.cost_terms);
        }
        return;
    }
    let mut reachable = Vec::with_capacity(a.queries);
    let mut attempts = 0usize;
    while reachable.len() < a.queries && attempts < a.queries * 1000 {
        attempts += 1;
        let len = 2 + rng.below(4) as usize;
        if let Some(p) = walk(&snap, &engine.table, &mut rng, len) {
            reachable.push(p);
        }
    }
    let random: Vec<_> = (0..a.queries)
        .map(|_| loop {
            let (s, t) = (pick_node(&snap, &mut rng), pick_node(&snap, &mut rng));
            if s != t {
                break (s, t);
            }
        })
        .collect();
    report.insert("walk_attempts".into(), json!(attempts));

    // what the start establishes (R7 closure), alone
    let mask = atlas_core::CURRENT_STATES_MASK & !EvidenceState::Disputed.bit();
    let (mut cms, mut csize, mut ccomplete) = (Vec::new(), Vec::new(), 0usize);
    for &(s, _) in reachable.iter().take(100) {
        let t = Instant::now();
        let d = atlas_paths::derive::closure(&snap, s, &[], mask, a.derive_budget);
        cms.push(t.elapsed().as_secs_f64() * 1000.0);
        csize.push(d.size as f64);
        ccomplete += d.complete as usize;
    }
    cms.sort_by(f64::total_cmp);
    csize.sort_by(f64::total_cmp);
    report.insert("closure".into(), json!({"starts": cms.len(), "complete": ccomplete,
        "ms": {"p50": pct(&cms, 0.5), "p95": pct(&cms, 0.95), "max": pct(&cms, 1.0)},
        "size": {"p50": pct(&csize, 0.5), "p95": pct(&csize, 0.95), "max": pct(&csize, 1.0)}}));
    eprintln!("closure: {}", report["closure"]);
    if std::env::var_os("BENCH_CLOSURE_ONLY").is_some() {
        println!("{}", serde_json::to_string_pretty(&Value::Object(report)).unwrap());
        return;
    }

    let opts = PathOptions { k: a.k, derive_budget: a.derive_budget, alt_budget_factor: a.alt_factor, alt_min_expansions: a.alt_min, concepts: a.concepts, ..Default::default() };
    let mut peak = mem();
    let mut per = serde_json::Map::new();
    let only = std::env::var("BENCH_OBJECTIVES").unwrap_or_default();
    for o in Objective::ALL {
        let name = serde_json::to_value(o).unwrap().as_str().unwrap().to_owned();
        if !only.is_empty() && !only.split(',').any(|x| x == name) {
            continue;
        }
        let r = json!({
            "reachable": run_class(&engine, &reachable, o, &a, &opts, &mut peak),
            "random": run_class(&engine, &random, o, &a, &opts, &mut peak),
        });
        eprintln!("{name}: {}", r);
        per.insert(name, r);
    }
    report.insert("k".into(), json!(a.k));
    report.insert("alt_budget".into(), json!({"factor": a.alt_factor, "min_expansions": a.alt_min}));
    report.insert("max_hops".into(), json!(a.max_hops));
    report.insert("objectives".into(), Value::Object(per));

    if std::env::var_os("BENCH_SKIP_EXTRA").is_some() {
        println!("{}", serde_json::to_string_pretty(&Value::Object(report)).unwrap());
        return;
    }
    // ablation on the reachable class, best_supported, k = 1
    let mut abl = serde_json::Map::new();
    for (bi, h, name) in [(true, true, "bidirectional-alt"), (true, false, "bidirectional-dijkstra"), (false, true, "alt-astar"), (false, false, "dijkstra")] {
        let o1 = PathOptions { k: 1, bidirectional: bi, heuristic: h, derive_budget: a.derive_budget, ..Default::default() };
        let sub = &reachable[..reachable.len().min(100)];
        abl.insert(name.into(), run_class(&engine, sub, Objective::BestSupported, &a, &o1, &mut peak));
    }
    report.insert("ablation_k1_best_supported_reachable".into(), Value::Object(abl));

    // throughput: all cores, reachable class, best_supported, default k
    let t2 = Instant::now();
    let done: usize = reachable
        .par_iter()
        .map(|&(s, t)| engine.find_with(&query(&snap, s, t, Objective::BestSupported, a.max_hops), &opts).map(|_| 1).unwrap_or(0))
        .sum();
    let secs = t2.elapsed().as_secs_f64();
    let m2 = mem();
    peak = (peak.0.max(m2.0), peak.1.max(m2.1));
    report.insert("throughput_best_supported_reachable".into(), json!({"queries": done, "secs": secs, "qps": done as f64 / secs}));
    report.insert("peak_sampled".into(), json!({"working_set_mib": mib(peak.0), "private_mib": mib(peak.1)}));
    println!("{}", serde_json::to_string_pretty(&Value::Object(report)).unwrap());
}
