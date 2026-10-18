//! Search benchmark: index build (time, peak memory, bytes) and query latency per
//! lane and fused, one query at a time and under N concurrent clients.
//!
//! ```text
//! bench_search real  --snapshot data/snapshot --db data/atlas.db --index data/index [--build]
//! bench_search synth --snapshot <1M snapshot dir> --index <dir> [--build] [--dense-only]
//! (--no-dense-synth skips the 384-dim exact-vs-HNSW comparison, ~8 min)
//! common: --queries 200 --clients 16 --out results.json
//! ```
//! `synth` writes deterministic synthetic text (Zipf vocabulary, ES/EN labels, Lean
//! identifiers, LaTeX/Lean formulas) for every node of a synthetic snapshot and builds
//! with the TF-IDF+SVD encoder; it also measures exact scan vs HNSW at 384 dims (the
//! neural encoder's width) on clustered synthetic vectors.  `real` builds with the
//! default encoder (neural if it loads) and samples queries from the registry text.

use atlas_core::{NodeIx, Snapshot};
use atlas_search::corpus::{self, Doc, Field, Text};
use atlas_search::{BuildOptions, DenseMethod, EncoderChoice, SearchIndex, SearchQuery, dense};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- utilities

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len())]
    }
    /// Zipf-like rank in `0..n` (s = 1): `n^u - 1`.
    fn zipf(&mut self, n: usize) -> usize {
        let u = (self.next() >> 11) as f64 / (1u64 << 53) as f64;
        ((n as f64).powf(u) - 1.0) as usize
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
}

/// Peak working set and private bytes of this process, sampled every 20 ms.
struct Peak {
    ws: Arc<AtomicU64>,
    private: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Peak {
    fn start() -> Peak {
        let (ws, private, stop) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)), Arc::new(AtomicBool::new(false)));
        let (w, p, s) = (ws.clone(), private.clone(), stop.clone());
        std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                if let Some(m) = memory_stats::memory_stats() {
                    w.fetch_max(m.physical_mem as u64, Ordering::Relaxed);
                    p.fetch_max(m.virtual_mem as u64, Ordering::Relaxed);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        Peak { ws, private, stop }
    }
    fn mib(self) -> Value {
        self.stop.store(true, Ordering::Relaxed);
        let m = |a: &AtomicU64| (a.load(Ordering::Relaxed) as f64 / (1u64 << 20) as f64).round();
        json!({"peak_working_set_mib": m(&self.ws), "peak_private_mib": m(&self.private)})
    }
}

fn mem_now() -> Value {
    let m = memory_stats::memory_stats().map_or((0, 0), |m| (m.physical_mem, m.virtual_mem));
    json!({"working_set_mib": m.0 >> 20, "private_mib": m.1 >> 20})
}

fn pct(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// Microsecond latencies -> {n, p50, p95, p99, max, mean} in ms.
fn summary(mut us: Vec<f64>) -> Value {
    us.sort_by(f64::total_cmp);
    let ms = |x: f64| (x / 10.0).round() / 100.0;
    let mean = us.iter().sum::<f64>() / us.len().max(1) as f64;
    json!({"n": us.len(), "p50_ms": ms(pct(&us, 0.5)), "p95_ms": ms(pct(&us, 0.95)), "p99_ms": ms(pct(&us, 0.99)),
           "max_ms": ms(us.last().copied().unwrap_or(f64::NAN)), "mean_ms": ms(mean)})
}

// ---------------------------------------------------------------- synthetic text

const NOUNS: [(&str, &str); 40] = [
    ("teorema", "theorem"), ("grupo", "group"), ("anillo", "ring"), ("campo", "field"), ("espacio", "space"),
    ("módulo", "module"), ("ideal", "ideal"), ("variedad", "manifold"), ("medida", "measure"), ("integral", "integral"),
    ("serie", "series"), ("sucesión", "sequence"), ("matriz", "matrix"), ("operador", "operator"), ("función", "function"),
    ("polinomio", "polynomial"), ("grafo", "graph"), ("categoría", "category"), ("functor", "functor"), ("haz", "sheaf"),
    ("topología", "topology"), ("norma", "norm"), ("producto", "product"), ("límite", "limit"), ("derivada", "derivative"),
    ("ecuación", "equation"), ("desigualdad", "inequality"), ("conjetura", "conjecture"), ("lema", "lemma"), ("álgebra", "algebra"),
    ("homología", "homology"), ("cohomología", "cohomology"), ("primo", "prime"), ("divisor", "divisor"), ("forma", "form"),
    ("curva", "curve"), ("superficie", "surface"), ("retículo", "lattice"), ("orden", "order"), ("conjunto", "set"),
];
const ADJS: [(&str, &str); 24] = [
    ("compacto", "compact"), ("conexo", "connected"), ("abeliano", "abelian"), ("finito", "finite"), ("libre", "free"),
    ("simple", "simple"), ("normal", "normal"), ("acotado", "bounded"), ("continuo", "continuous"), ("lineal", "linear"),
    ("diferenciable", "differentiable"), ("analítico", "analytic"), ("algebraico", "algebraic"), ("proyectivo", "projective"),
    ("noetheriano", "noetherian"), ("separable", "separable"), ("completo", "complete"), ("exacto", "exact"),
    ("regular", "regular"), ("convexo", "convex"), ("aleatorio", "random"), ("discreto", "discrete"), ("denso", "dense"),
    ("unitario", "unitary"),
];
const NAMES: [&str; 32] = [
    "Euler", "Gauss", "Riemann", "Hilbert", "Noether", "Cauchy", "Fermat", "Galois", "Abel", "Jacobi", "Lagrange",
    "Fourier", "Banach", "Sylow", "Lebesgue", "Poincaré", "Kolmogorov", "Grothendieck", "Cantor", "Dedekind",
    "Weierstrass", "Hausdorff", "Zorn", "Stokes", "Green", "Laplace", "Ramanujan", "Hardy", "Turing", "Gödel",
    "Pitágoras", "Euclides",
];
const LEAN_NS: [&str; 16] = [
    "Nat", "Int", "Real", "Complex", "Finset", "Set", "List", "Polynomial", "MeasureTheory", "Matrix",
    "LinearMap", "Subgroup", "Ideal", "TopologicalSpace", "Filter", "ZMod",
];
const VARS: [&str; 16] = ["a", "b", "c", "x", "y", "z", "n", "m", "k", "p", "q", "f", "g", "t", "u", "v"];
const RENAME: [&str; 16] = ["r", "s", "w", "l", "h", "o", "α", "β", "γ", "λ", "μ", "ν", "ρ", "σ", "τ", "φ"];

/// A pseudo-word vocabulary of `n` words (syllable combinations); rank = Zipf frequency.
fn vocabulary(n: usize) -> Vec<String> {
    const SYL: [&str; 24] = [
        "ra", "to", "mi", "ne", "lo", "sa", "ki", "pu", "de", "fo", "ga", "hi", "ju", "ve", "ri", "co", "ba", "ze",
        "ti", "mo", "nu", "pe", "la", "xi",
    ];
    let mut out: Vec<String> = NOUNS.iter().flat_map(|(e, n)| [e.to_string(), n.to_string()]).collect();
    out.extend(ADJS.iter().flat_map(|(e, n)| [e.to_string(), n.to_string()]));
    let mut i = 0usize;
    while out.len() < n {
        let (a, b, c) = (i % 24, (i / 24) % 24, (i / 576) % 24);
        out.push(format!("{}{}{}", SYL[a], SYL[b], if i >= 576 { SYL[c] } else { "" }));
        i += 1;
    }
    out
}

fn expr(r: &mut Rng, depth: usize, vars: &[&str]) -> String {
    if depth == 0 || r.below(4) == 0 {
        return if r.below(5) == 0 { (1 + r.below(9)).to_string() } else { r.pick(vars).to_string() };
    }
    let (a, b) = (expr(r, depth - 1, vars), expr(r, depth - 1, vars));
    match r.below(9) {
        0 => format!("{a} + {b}"),
        1 => format!("{a} - {b}"),
        2 => format!("{a} {b}"),
        3 => format!("{{{a}}}^{{{}}}", 2 + r.below(3)),
        4 => format!("\\frac{{{a}}}{{{b}}}"),
        5 => format!("\\sqrt{{{a}}}"),
        6 => format!("\\sum_{{i=1}}^{{n}} {a}"),
        7 => format!("\\int_0^1 {a} \\, dx"),
        _ => format!("f({a}, {b})"),
    }
}

fn formula(r: &mut Rng) -> String {
    let lhs = expr(r, 3, &VARS);
    let rhs = expr(r, 3, &VARS);
    let rel = *r.pick(&["=", "\\le", "<", "="]);
    format!("{lhs} {rel} {rhs}")
}

fn lean_ident(r: &mut Rng, i: usize) -> String {
    let (n, _) = *r.pick(&NOUNS);
    let (_, a) = *r.pick(&ADJS);
    format!("{}.{}_{}_{}", r.pick(&LEAN_NS), NOUNS[i % NOUNS.len()].1, a, if n.len() > 5 { "of_le" } else { "iff" })
}

fn words(r: &mut Rng, vocab: &[String], k: usize) -> String {
    (0..k).map(|_| vocab[r.zipf(vocab.len())].as_str()).collect::<Vec<_>>().join(" ")
}

fn synth_doc(node: NodeIx, vocab: &[String]) -> Doc {
    let mut r = Rng(0xA71A5 ^ (node as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
    let t = |field, lang: &str, text: String| Text { field, lang: lang.into(), text };
    let (ne, nn) = *r.pick(&NOUNS);
    let (ae, an) = *r.pick(&ADJS);
    let name = *r.pick(&NAMES);
    let mut texts = vec![t(Field::Label, "es", format!("{ne} {ae} de {name}")), t(Field::Label, "en", format!("{name}'s {an} {nn}"))];
    if r.below(10) < 4 {
        texts.push(t(Field::Alias, "und", lean_ident(&mut r, node as usize)));
    }
    if r.below(10) < 4 {
        let k = 8 + r.below(24);
        texts.push(t(Field::Description, if r.below(2) == 0 { "es" } else { "en" }, words(&mut r, vocab, k)));
    }
    match r.below(10) {
        0..=3 => {
            let (v, w) = (*r.pick(&VARS), *r.pick(&VARS));
            let f = formula(&mut r).replace("\\le", "≤").replace('\\', "");
            texts.push(t(Field::Statement, "und", format!("theorem {} ({v} {w} : ℕ) : {f}", lean_ident(&mut r, node as usize))));
        }
        4..=6 => {
            let k = 10 + r.below(20);
            texts.push(t(Field::Statement, "en", words(&mut r, vocab, k)));
        }
        _ => {}
    }
    if r.below(20) == 0 {
        texts.push(t(Field::Formula, "und", formula(&mut r)));
    }
    texts.push(t(Field::ExternalId, "und", format!("wikidata:Q{}", 1_000_000 + node)));
    Doc { node, texts }
}

// ---------------------------------------------------------------- queries

#[derive(Clone)]
struct Q {
    class: &'static str,
    text: String,
}

/// Rename standalone single-letter variables (`x^2+y^2` -> `ρ^2+σ^2`), keeping `f`, `g`.
fn rename_vars(f: &str, r: &mut Rng) -> String {
    let shift = 1 + r.below(RENAME.len() - 1);
    let cs: Vec<char> = f.chars().collect();
    cs.iter()
        .enumerate()
        .map(|(i, &c)| {
            let alone = !(i > 0 && (cs[i - 1].is_alphanumeric() || cs[i - 1] == '\\'))
                && !cs.get(i + 1).is_some_and(|n| n.is_alphanumeric());
            match VARS.iter().position(|v| v.starts_with(c)) {
                Some(p) if alone && c != 'f' && c != 'g' => RENAME[(p + shift) % RENAME.len()].to_string(),
                _ => c.to_string(),
            }
        })
        .collect()
}

/// Queries of four classes sampled from documents (seeded).
fn sample_queries(docs: &[Doc], n: usize, seed: u64) -> Vec<Q> {
    let mut r = Rng(seed);
    let mut out = Vec::new();
    let first = |d: &Doc, f: Field| d.of(f).next().map(|t| t.text.clone());
    let mut tries = 0;
    let (mut named, mut form, mut nl, mut cross) = (0, 0, 0, 0);
    while (named < n || form < n || nl < n || cross < n) && tries < n * 2000 {
        tries += 1;
        let d = &docs[r.below(docs.len())];
        if named < n {
            if let Some(l) = d.of(Field::Label).nth(r.below(2)).or_else(|| d.of(Field::Label).next()) {
                out.push(Q { class: "named_object", text: l.text.clone() });
                named += 1;
            }
        }
        if form < n {
            let src = first(d, Field::Formula).or_else(|| {
                first(d, Field::Statement).filter(|s| s.starts_with("theorem ")).and_then(|s| s.rsplit_once(" : ").map(|x| x.1.to_string()))
            });
            if let Some(f) = src.filter(|f| f.chars().count() >= 5 && f.len() < 300) {
                out.push(Q { class: "formula", text: rename_vars(&f, &mut r) });
                form += 1;
            }
        }
        if nl < n {
            let src = first(d, Field::Description).or_else(|| first(d, Field::Statement).filter(|s| !s.starts_with("theorem ")));
            let ws: Vec<String> = src.map(|s| s.split_whitespace().map(String::from).collect()).unwrap_or_default();
            if ws.len() >= 8 && !corpus_formal(&ws) {
                let start = r.below(ws.len() - 7);
                out.push(Q { class: "natural_language", text: ws[start..start + 8].join(" ") });
                nl += 1;
            }
        }
        if cross < n {
            let other = &docs[r.below(docs.len())];
            if let (Some(a), Some(b)) = (d.of(Field::Label).next(), other.of(Field::Label).next()) {
                let short = |s: &str| s.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
                out.push(Q { class: "cross_domain", text: format!("relación entre {} y {}", short(&a.text), short(&b.text)) });
                cross += 1;
            }
        }
    }
    out
}

fn corpus_formal(ws: &[String]) -> bool {
    atlas_search::formula::looks_formal(&ws.join(" "))
}

// ---------------------------------------------------------------- measurement

const CLASSES: [&str; 4] = ["named_object", "formula", "natural_language", "cross_domain"];
const MODES: [&str; 4] = ["lexical", "formula", "semantic", "fused"];

fn query_for(q: &Q, mode: &str) -> SearchQuery {
    let text = if mode == "fused" { q.text.clone() } else { format!("{} lane:{mode}", q.text) };
    SearchQuery { q: text, lang: "es".into(), types: vec![], limit: 20 }
}

/// One query at a time: latency per (class, mode), plus pure lane times from the
/// fused runs and the routes chosen.
fn sequential(idx: &SearchIndex, snap: &Snapshot, qs: &[Q]) -> Value {
    let mut out = serde_json::Map::new();
    for &class in &CLASSES {
        let cq: Vec<&Q> = qs.iter().filter(|q| q.class == class).collect();
        let mut per_mode = serde_json::Map::new();
        let mut lane_us: [Vec<f64>; 3] = Default::default();
        let mut encode_us = Vec::new();
        let mut routes: std::collections::BTreeMap<String, usize> = Default::default();
        let mut empty = 0;
        for &mode in &MODES {
            let mut lat = Vec::with_capacity(cq.len());
            for q in &cq {
                let sq = query_for(q, mode);
                let t = Instant::now();
                let (r, e) = idx.search_explained(snap, &sq).expect("search");
                lat.push(t.elapsed().as_secs_f64() * 1e6);
                if mode == "fused" {
                    *routes.entry(format!("{:?}", e.route.kind)).or_default() += 1;
                    empty += r.hits.is_empty() as usize;
                    for (i, l) in e.lanes.iter().enumerate() {
                        if l.weight > 0.0 {
                            lane_us[i].push(l.micros as f64);
                        }
                    }
                    if let Some(n) = e.lanes.get(2).and_then(|l| l.note.as_deref()) {
                        if let Some(v) = n.strip_prefix("encode ").and_then(|s| s.strip_suffix(" us")) {
                            encode_us.push(v.parse().unwrap_or(0.0));
                        }
                    }
                }
            }
            per_mode.insert(mode.into(), summary(lat));
        }
        let lane_names = ["lexical", "formula", "semantic"];
        let inside: serde_json::Map<String, Value> =
            lane_names.iter().zip(lane_us).map(|(n, v)| (n.to_string(), summary(v))).collect();
        out.insert(class.into(), json!({"end_to_end": per_mode, "lane_inside_fused": inside,
            "semantic_query_encode": summary(encode_us), "routes": routes, "fused_empty": empty}));
    }
    Value::Object(out)
}

/// `clients` threads issuing fused queries for `secs` seconds.
fn concurrent(idx: &SearchIndex, snap: &Snapshot, qs: &[Q], clients: usize, secs: f64) -> Value {
    let stop = AtomicBool::new(false);
    let lat: Vec<Vec<f64>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..clients)
            .map(|c| {
                let stop = &stop;
                s.spawn(move || {
                    let mut v = Vec::new();
                    let mut i = c * 7919;
                    while !stop.load(Ordering::Relaxed) {
                        let q = &qs[i % qs.len()];
                        i += 1;
                        let t = Instant::now();
                        idx.search(snap, &query_for(q, "fused")).expect("search");
                        v.push(t.elapsed().as_secs_f64() * 1e6);
                    }
                    v
                })
            })
            .collect();
        std::thread::sleep(Duration::from_secs_f64(secs));
        stop.store(true, Ordering::Relaxed);
        hs.into_iter().map(|h| h.join().expect("client")).collect()
    });
    let all: Vec<f64> = lat.into_iter().flatten().collect();
    let qps = all.len() as f64 / secs;
    json!({"clients": clients, "secs": secs, "queries": all.len(), "qps": qps.round(), "latency": summary(all)})
}

/// recall@10 two ways: by id, and by score (a returned row counts if its exact int8
/// cosine reaches the 10th exact score; duplicate vectors make id overlap understate it).
fn recall_hits(v: &dense::Vectors, q: &[f32], got: &[(NodeIx, f32)], truth: &[(NodeIx, f32)]) -> (usize, usize) {
    let kth = truth.last().map_or(f32::NEG_INFINITY, |x| x.1);
    let (qc, qs) = dense::quantize(q);
    let exact = |n: NodeIx| {
        let d: i32 = v.row(n).iter().zip(&qc).map(|(&a, &b)| a as i32 * b as i32).sum();
        d as f32 * v.scales()[n as usize] * qs
    };
    let by_id = got.iter().filter(|x| truth.iter().any(|y| y.0 == x.0)).count();
    let by_score = got.iter().take(truth.len()).filter(|x| exact(x.0) >= kth - 1e-5 * kth.abs().max(1.0)).count();
    (by_id, by_score)
}

/// Exact scan vs HNSW of the index's own vectors: latency and recall@10.
fn dense_compare(idx: &mut SearchIndex, qs: &[Q]) -> Value {
    let vecs: Vec<Vec<f32>> = qs.iter().filter_map(|q| idx.encode_query(&q.text).ok()).collect();
    let mut res = serde_json::Map::new();
    let mut truth = Vec::new();
    idx.set_dense_method(DenseMethod::Brute).expect("brute");
    let mut lat = Vec::new();
    for v in &vecs {
        let t = Instant::now();
        truth.push(idx.dense_search(v, 10, None).expect("brute"));
        lat.push(t.elapsed().as_secs_f64() * 1e6);
    }
    res.insert("brute_parallel".into(), summary(lat));
    if idx.set_dense_method(DenseMethod::Hnsw).is_ok() {
        for ef in [32usize, 96, 256] {
            idx.set_hnsw_expansion(ef);
            let (mut lat, mut hit, mut hit_s, mut tot) = (Vec::new(), 0usize, 0usize, 0usize);
            for (v, tr) in vecs.iter().zip(&truth) {
                let t = Instant::now();
                let h = idx.dense_search(v, 10, None).expect("hnsw");
                lat.push(t.elapsed().as_secs_f64() * 1e6);
                let (a, b) = recall_hits(idx.vectors(), v, &h, tr);
                (hit, hit_s, tot) = (hit + a, hit_s + b, tot + tr.len());
            }
            let t = tot.max(1) as f64;
            res.insert(format!("hnsw_ef{ef}"), json!({"latency": summary(lat), "recall_at_10_ids": hit as f64 / t,
                "recall_at_10": hit_s as f64 / t}));
        }
        idx.set_hnsw_expansion(dense::HNSW_EXPANSION_SEARCH);
    }
    json!({"queries": vecs.len(), "methods": res})
}

/// Exact scan vs HNSW on `n` clustered synthetic vectors of `dim` dims (no encoder).
fn dense_synthetic(dir: &PathBuf, n: usize, dim: usize, queries: usize) -> Value {
    let mut r = Rng(42);
    let centroids: Vec<Vec<f32>> = (0..2048).map(|_| (0..dim).map(|_| r.unit()).collect()).collect();
    let t = Instant::now();
    let rows: Vec<(Vec<i8>, f32)> = (0..n)
        .map(|i| {
            let c = &centroids[(i * 2654435761) % centroids.len()];
            let mut v: Vec<f32> = c.iter().map(|x| x + 0.6 * r.unit()).collect();
            atlas_search::encoder::normalize(&mut v);
            dense::quantize(&v)
        })
        .collect();
    let gen_s = t.elapsed().as_secs_f64();
    std::fs::create_dir_all(dir).expect("dir");
    let nodes: Vec<NodeIx> = (0..n as NodeIx).collect();
    let vp = dir.join("vectors-synth.bin");
    let bytes = dense::write(&nodes, &rows, n, dim, &vp).expect("write");
    drop(rows);
    let vec = dense::Vectors::open(&vp).expect("open");
    let peak = Peak::start();
    let t = Instant::now();
    let hp = dir.join("hnsw-synth.usearch");
    let hbytes = dense::Hnsw::build(&vec, &hp, std::thread::available_parallelism().map_or(4, |x| x.get())).expect("hnsw");
    let hnsw_s = t.elapsed().as_secs_f64();
    let mem = peak.mib();
    let h = dense::Hnsw::open(&hp).expect("open hnsw");
    let qv: Vec<Vec<f32>> = (0..queries)
        .map(|i| {
            let mut v = vec.row_f32(((i * 7_919_003) % n) as NodeIx);
            v.iter_mut().for_each(|x| *x += 0.02 * r.unit());
            atlas_search::encoder::normalize(&mut v);
            v
        })
        .collect();
    let mut out = serde_json::Map::new();
    let mut truth = Vec::new();
    for (name, par) in [("brute_parallel", true), ("brute_1thread", false)] {
        let mut lat = Vec::new();
        for q in &qv {
            let t = Instant::now();
            let r = vec.brute(q, 10, par, &|_| true);
            lat.push(t.elapsed().as_secs_f64() * 1e6);
            if par {
                truth.push(r);
            }
        }
        out.insert(name.into(), summary(lat));
    }
    for ef in [32usize, 96, 256] {
        h.set_expansion(ef);
        let (mut lat, mut hit, mut hit_s) = (Vec::new(), 0usize, 0usize);
        for (q, tr) in qv.iter().zip(&truth) {
            let t = Instant::now();
            let r = h.search(q, 10, None).expect("hnsw");
            lat.push(t.elapsed().as_secs_f64() * 1e6);
            let (a, b) = recall_hits(&vec, q, &r, tr);
            (hit, hit_s) = (hit + a, hit_s + b);
        }
        let t = (10 * qv.len()) as f64;
        out.insert(format!("hnsw_ef{ef}"), json!({"latency": summary(lat), "recall_at_10_ids": hit as f64 / t,
            "recall_at_10": hit_s as f64 / t}));
    }
    drop(h);
    drop(vec);
    let _ = std::fs::remove_file(&vp);
    let _ = std::fs::remove_file(&hp);
    json!({"vectors": n, "dim": dim, "generate_s": gen_s, "vectors_bytes": bytes, "hnsw_build_s": hnsw_s,
           "hnsw_bytes": hbytes, "hnsw_build_memory": mem, "queries": queries, "methods": out})
}

// ---------------------------------------------------------------- main

struct Args {
    mode: String,
    snapshot: PathBuf,
    db: PathBuf,
    index: PathBuf,
    build: bool,
    dense_only: bool,
    no_dense_synth: bool,
    queries: usize,
    clients: usize,
    secs: f64,
    out: Option<PathBuf>,
}

fn args() -> Args {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let has = |k: &str| a.iter().any(|x| x == k);
    Args {
        mode: a.first().cloned().unwrap_or_else(|| "real".into()),
        snapshot: get("--snapshot").unwrap_or_else(|| "data/snapshot".into()).into(),
        db: get("--db").unwrap_or_else(|| "data/atlas.db".into()).into(),
        index: get("--index").unwrap_or_else(|| "data/index".into()).into(),
        build: has("--build"),
        dense_only: has("--dense-only"),
        no_dense_synth: has("--no-dense-synth"),
        queries: get("--queries").and_then(|x| x.parse().ok()).unwrap_or(200),
        clients: get("--clients").and_then(|x| x.parse().ok()).unwrap_or(16),
        secs: get("--secs").and_then(|x| x.parse().ok()).unwrap_or(10.0),
        out: get("--out").map(PathBuf::from),
    }
}

fn main() {
    let a = args();
    let mut report = serde_json::Map::new();
    report.insert("mode".into(), json!(a.mode));
    report.insert("threads".into(), json!(std::thread::available_parallelism().map_or(0, |x| x.get())));
    let emit = |report: &serde_json::Map<String, Value>| {
        let s = serde_json::to_string_pretty(report).expect("json");
        if let Some(p) = &a.out {
            std::fs::write(p, &s).expect("write results");
        }
        s
    };
    let synth = a.mode == "synth";
    if synth && !a.no_dense_synth {
        report.insert("dense_384".into(), dense_synthetic(&a.index, 1_000_000, 384, a.queries));
        eprintln!("dense_384 done");
        emit(&report);
        if a.dense_only {
            println!("{}", emit(&report));
            return;
        }
    }
    let snap = Snapshot::open(&a.snapshot).expect("snapshot");
    report.insert("snapshot".into(), json!({"nodes": snap.num_nodes(), "hash": &snap.content_hash_hex()[..16]}));
    let t = Instant::now();
    let docs: Vec<Doc> = if synth {
        let vocab = vocabulary(20_000);
        use rayon::prelude::*;
        (0..snap.num_nodes()).into_par_iter().map(|n| synth_doc(n, &vocab)).collect()
    } else {
        corpus::load(&snap, &a.db).expect("corpus")
    };
    let n_texts: usize = docs.iter().map(|d| d.texts.len()).sum();
    let n_chars: usize = docs.iter().flat_map(|d| &d.texts).map(|t| t.text.len()).sum();
    report.insert("corpus".into(), json!({"documents": docs.len(), "texts": n_texts, "text_bytes": n_chars,
        "load_s": t.elapsed().as_secs_f64()}));
    if a.build {
        let opts = if synth { BuildOptions { encoder: EncoderChoice::Lsi, ..BuildOptions::default() } } else { BuildOptions::default() };
        let base = mem_now();
        let peak = Peak::start();
        let r = SearchIndex::build_from_docs(&snap, &docs, &a.index, &opts).expect("build");
        report.insert("build".into(), json!({"report": r, "memory_before": base, "memory": peak.mib()}));
        eprintln!("build done in {:.1}s", r.secs);
        emit(&report);
    }
    let qs = sample_queries(&docs, a.queries, 7);
    let samples: Value = CLASSES
        .iter()
        .map(|c| (c.to_string(), json!(qs.iter().filter(|q| q.class == *c).take(3).map(|q| &q.text).collect::<Vec<_>>())))
        .collect::<serde_json::Map<_, _>>()
        .into();
    report.insert("query_samples".into(), samples);
    drop(docs);
    let before = mem_now();
    let t = Instant::now();
    let mut idx = SearchIndex::open(&snap, &a.index).expect("open index");
    report.insert("open".into(), json!({"ms": t.elapsed().as_secs_f64() * 1e3, "memory_before": before, "memory_after": mem_now(),
        "status": idx.status()}));
    // warm-up (page cache, ONNX session init)
    for q in qs.iter().take(40) {
        let _ = idx.search(&snap, &query_for(q, "fused"));
    }
    report.insert("sequential".into(), sequential(&idx, &snap, &qs));
    eprintln!("sequential done");
    emit(&report);
    let default_method = idx.dense_method();
    report.insert("dense".into(), dense_compare(&mut idx, &qs));
    idx.set_dense_method(default_method).expect("default dense method");
    report.insert("concurrent_1".into(), concurrent(&idx, &snap, &qs, 1, a.secs));
    report.insert(format!("concurrent_{}", a.clients), concurrent(&idx, &snap, &qs, a.clients, a.secs));
    report.insert("memory_end".into(), mem_now());
    report.insert("env".into(), json!({"ATLAS_QUERY_SESSIONS": std::env::var("ATLAS_QUERY_SESSIONS").ok(),
        "ATLAS_QUERY_THREADS": std::env::var("ATLAS_QUERY_THREADS").ok()}));
    println!("{}", emit(&report));
}
