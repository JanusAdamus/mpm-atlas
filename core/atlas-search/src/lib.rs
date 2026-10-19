//! atlas-search: hybrid retrieval over one atlas snapshot.
//!
//! Three lanes, fused by weighted Reciprocal Rank Fusion (`score = Σ w_lane / (60 + rank)`):
//! - lexical: tantivy BM25 over labels, aliases, external ids, descriptions and
//!   statements with Spanish, English and Lean-identifier analysers (`lexical.rs`);
//! - formula: LaTeX / MathML / Lean expressions as operator trees, Tangent-style path
//!   and leaf-pair features with variable unification, tf-idf cosine (`formula/`);
//! - semantic: a multilingual sentence encoder (multilingual-e5-small via fastembed)
//!   or TF-IDF+SVD, int8 vectors searched by HNSW (usearch) or an exact scan (`dense.rs`).
//!
//! A router (`router.rs`) classifies the query (named object, formula, natural
//! language, cross-domain) and sets the lane weights; inline filters (`type:`,
//! `assume:`, `lane:`) and `SearchQuery.types` restrict the hits.  Every hit reports
//! each lane's rank and raw score (`lanes`) so a ranking can be diagnosed.
//!
//! Text that is not in the snapshot (aliases, descriptions, statements, formulas,
//! external ids) is read from the registry (`data/atlas.db`) at build time.  Indexes
//! are derived data: `build` writes `<out>/<snapshot hash[..16]>/`, `open` refuses an
//! index built for another snapshot.

pub mod corpus;
pub mod dense;
pub mod encoder;
pub mod formula;
pub mod lexical;
pub mod router;
pub mod text;
pub mod textstore;

use ahash::{AHashMap, AHashSet};
use atlas_core::{EntityType, Error, NodeIx, Result, Role, Snapshot};
use corpus::{Doc, Field};
use encoder::{Encoder, Lsi};
use router::{Filters, Lane, Route, RouteKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tantivy::tokenizer::TokenizerManager;

/// Parsed `/api/search?q=&lang=es&types=statement,concept&limit=20`.
#[derive(Clone, Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_lang")]
    pub lang: String,
    #[serde(default)]
    pub types: Vec<EntityType>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_lang() -> String {
    "es".into()
}

fn default_limit() -> usize {
    20
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub lang: String,
    pub limit: usize,
    pub truncated: bool,
    pub hits: Vec<Hit>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Hit {
    pub id: String,
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    pub label: String,
    pub matched: Matched,
    /// Weighted RRF score (higher is better): `Σ w_lane / (RRF_K + rank_lane)`.
    pub score: f64,
    pub lanes: Lanes,
}

#[derive(Clone, Debug, Serialize)]
pub struct Matched {
    pub field: String,
    pub lang: String,
    pub text: String,
}

/// Contribution of each retrieval lane, for diagnosis.  `None` = lane not used or
/// the hit was not in that lane's candidates.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Lanes {
    pub lexical: Option<LaneScore>,
    pub formula: Option<LaneScore>,
    pub semantic: Option<LaneScore>,
    /// The lexical query over concept-layer nodes only (everyday names, see `router.rs`).
    pub concept: Option<LaneScore>,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct LaneScore {
    pub rank: u32,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct BuildReport {
    pub index_dir: PathBuf,
    pub documents: u64,
    pub secs: f64,
    /// Seconds per build phase.
    pub phases: BTreeMap<String, f64>,
    pub encoder: String,
    pub dim: usize,
    /// Documents run through the encoder in this build (the rest came from the cache).
    pub encoded: u64,
    pub cache_hits: u64,
    pub formula: formula::FormulaBuildStats,
    /// Bytes on disk per component.
    pub bytes: BTreeMap<String, u64>,
}

/// RRF constant (Cormack, Clarke & Büttcher 2009).
pub const RRF_K: f64 = 60.0;
/// Bonus of a hit whose label or alias equals the query: what rank 1 in a lane of weight 1 gives.
const EXACT_BONUS: f64 = 1.0 / (RRF_K + 1.0);
const INDEX_FORMAT: u32 = 1;
const MAX_MATCHED_CHARS: usize = 280;
const MAX_LIMIT: usize = 1000;
/// Candidates per lane when post-filters (assumptions) may drop many of them.
const FILTERED_CANDIDATES: usize = 2000;
// Semantic search defaults to the exact scan; `ATLAS_DENSE=hnsw` uses the HNSW graph.
// Measured 2026-10-17 (16 cores, bench-results/): the scan is fast alone (p50 1.7 ms on
// 287k x 384 real e5 vectors, 2.1 ms on 1M x 128 LSI) and HNSW ef 96 recalls only
// 0.75 / 0.35 of its top 10.  HNSW was the default because the scan fills the rayon pool
// that other lanes and concurrent queries share (1M synthetic index, 16 clients: fused
// p99 81 ms -> 1.6 s).  Measured 2026-10-18 on the real 320k-node snapshot: with HNSW,
// AtlasBench v3 en/text nDCG@10 moved with each rebuilt graph (test 0.787 -> 0.771 with
// the same code); with the scan, dev en/text 0.754 -> 0.803.  Quality wins at this size;
// switch back past a few million vectors or if search p95 under load matters more.

// ---------------------------------------------------------------- build

#[derive(Clone, Debug, PartialEq)]
pub enum EncoderChoice {
    /// Neural if it is compiled in and loads, else TF-IDF + SVD.
    Auto,
    Lsi,
    Neural(String),
}

#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub encoder: EncoderChoice,
    /// Threads for tantivy and HNSW construction (0 = all cores).
    pub threads: usize,
    /// Also build the HNSW graph (the exact scan always works).
    pub hnsw: bool,
    /// ONNX sessions x intra-op threads for document encoding.
    pub sessions: usize,
    pub intra_threads: usize,
}

impl Default for BuildOptions {
    /// `ATLAS_ENCODER` = `lsi`, `neural`, or a model name overrides `Auto`.
    fn default() -> Self {
        let encoder = match std::env::var("ATLAS_ENCODER").as_deref() {
            Ok("lsi") => EncoderChoice::Lsi,
            Ok("neural") => EncoderChoice::Neural(encoder_default_model().into()),
            Ok(m) if !m.is_empty() => EncoderChoice::Neural(m.into()),
            _ => EncoderChoice::Auto,
        };
        // measured (probe_embed, 16 cores): 2x8, 4x4, 8x2 sessions x threads all ~300-370 texts/s
        BuildOptions { encoder, threads: 0, hnsw: true, sessions: 4, intra_threads: 4 }
    }
}

fn encoder_default_model() -> &'static str {
    #[cfg(feature = "neural")]
    {
        encoder::neural::DEFAULT_MODEL
    }
    #[cfg(not(feature = "neural"))]
    {
        "lsi"
    }
}

#[derive(Serialize, Deserialize)]
struct Meta {
    format: u32,
    snapshot_hash: String,
    num_nodes: u64,
    documents: u64,
    encoder_kind: String,
    encoder_name: String,
    dim: usize,
    hnsw: bool,
    #[serde(default)]
    build: serde_json::Value,
}

fn io(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |e| Error::io(path, e)
}

fn threads_of(n: usize) -> usize {
    if n == 0 { std::thread::available_parallelism().map_or(4, |n| n.get()) } else { n }
}

fn choose_encoder(opts: &BuildOptions, texts: &[String], dir: &Path) -> Result<Encoder> {
    let lsi = |texts: &[String]| -> Result<Encoder> {
        let l = Lsi::fit(texts, encoder::LSI_DIM, 0xA71A5);
        l.save(&dir.join("lsi.bin"))?;
        Ok(Encoder::Lsi(l))
    };
    match &opts.encoder {
        EncoderChoice::Lsi => lsi(texts),
        #[cfg(feature = "neural")]
        EncoderChoice::Neural(m) => {
            Ok(Encoder::Neural(encoder::neural::Neural::load(m, opts.sessions, opts.intra_threads)?))
        }
        #[cfg(not(feature = "neural"))]
        EncoderChoice::Neural(_) => Err(Error::BadRequest("built without the `neural` feature".into())),
        EncoderChoice::Auto => {
            #[cfg(feature = "neural")]
            match encoder::neural::Neural::load(encoder::neural::DEFAULT_MODEL, opts.sessions, opts.intra_threads) {
                Ok(n) => return Ok(Encoder::Neural(n)),
                Err(e) => eprintln!("atlas-search: neural encoder unavailable ({e}); using TF-IDF + SVD"),
            }
            lsi(texts)
        }
    }
}

fn dir_len(p: &Path) -> u64 {
    std::fs::read_dir(p).map_or(0, |rd| rd.flatten().map(|e| e.metadata().map_or(0, |m| m.len())).sum())
}

/// Encode `docs` (embedding cache for neural encoders), returning (codes, encoded, cache hits).
/// Per-document int8 vector and its scale, plus (cache hits, newly encoded).
type Embedded = (Vec<(Vec<i8>, f32)>, u64, u64);

fn embed(enc: &Encoder, docs: &[Doc], cache_path: &Path) -> Result<Embedded> {
    let texts: Vec<String> = docs.iter().map(encoder::doc_text).collect();
    let dim = enc.encode_query("dimension probe")?.len();
    let use_cache = enc.kind() != "lsi"; // an LSI projection is refitted per build
    let name = enc.name();
    let mut cache = if use_cache { dense::EmbCache::load(cache_path, dim) } else { dense::EmbCache::new(dim) };
    let keys: Vec<[u8; 16]> = texts.iter().map(|t| dense::cache_key(&name, t)).collect();
    let miss: Vec<usize> = (0..texts.len()).filter(|&i| cache.get(&keys[i]).is_none()).collect();
    let miss_texts: Vec<String> = miss.iter().map(|&i| texts[i].clone()).collect();
    let t = Instant::now();
    let step = (miss_texts.len() / 20).max(10_000);
    let next_report = std::sync::atomic::AtomicUsize::new(step);
    let progress = |done: usize| {
        if done >= next_report.load(std::sync::atomic::Ordering::Relaxed) {
            next_report.fetch_add(step, std::sync::atomic::Ordering::Relaxed);
            let s = t.elapsed().as_secs_f64();
            eprintln!("atlas-search: encoded {done}/{} ({:.0}/s)", miss_texts.len(), done as f64 / s.max(1e-9));
        }
    };
    let vecs = enc.encode_docs(&miss_texts, &progress)?;
    for (&i, v) in miss.iter().zip(&vecs) {
        cache.insert(keys[i], dense::quantize(v));
    }
    if use_cache && !miss.is_empty() {
        cache.save(cache_path)?;
    }
    let codes = keys.iter().map(|k| cache.get(k).cloned().unwrap_or_else(|| (vec![0; dim], 0.0))).collect();
    Ok((codes, miss.len() as u64, (texts.len() - miss.len()) as u64))
}

fn sanitize(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect()
}

// ---------------------------------------------------------------- the index

/// How the semantic lane finds neighbours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DenseMethod {
    /// Approximate (HNSW graph); used when the graph exists and `ATLAS_DENSE=hnsw`.
    Hnsw,
    /// Exact int8 scan over all vectors, parallel (the default).
    Brute,
}

/// Search indexes built for one snapshot (keyed by its content hash).
pub struct SearchIndex {
    dir: PathBuf,
    meta: Meta,
    lexical: lexical::LexicalIndex,
    formula: formula::FormulaIndex,
    texts: textstore::TextStore,
    vectors: dense::Vectors,
    hnsw: Option<dense::Hnsw>,
    encoder: Option<Encoder>,
    encoder_error: Option<String>,
    dense_method: DenseMethod,
    tokenizers: TokenizerManager,
    /// Node bitset of the concept layer (`Snapshot::concepts`), for the concept lane.
    concepts: Arc<Vec<u64>>,
}

/// Per-lane timing and candidate counts of one query.
#[derive(Clone, Debug, Serialize)]
pub struct LaneRun {
    pub lane: Lane,
    pub weight: f64,
    pub candidates: usize,
    pub micros: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// What the engine did with a query (not part of `/api/search`).
#[derive(Clone, Debug, Serialize)]
pub struct Explain {
    pub text: String,
    pub route: Route,
    pub filters: Filters,
    pub ignored: Vec<String>,
    pub lanes: Vec<LaneRun>,
    pub dense_method: DenseMethod,
    pub micros: u64,
}

struct Cand {
    score: f64,
    lanes: [Option<(u32, f32)>; 4],
}

impl SearchIndex {
    /// Build every lane's index for `snapshot` from the registry at `db` into
    /// `out_dir/<hash16>/`, with `BuildOptions::default()`.
    pub fn build(snapshot: &Snapshot, db: &Path, out_dir: &Path) -> Result<BuildReport> {
        Self::build_with(snapshot, db, out_dir, &BuildOptions::default())
    }

    pub fn build_with(snapshot: &Snapshot, db: &Path, out_dir: &Path, opts: &BuildOptions) -> Result<BuildReport> {
        let t = Instant::now();
        let docs = corpus::load(snapshot, db)?;
        let corpus_secs = t.elapsed().as_secs_f64();
        let mut r = Self::build_from_docs(snapshot, &docs, out_dir, opts)?;
        r.phases.insert("corpus".into(), corpus_secs);
        r.secs += corpus_secs;
        Ok(r)
    }

    /// Build from documents already in memory (benchmarks with synthetic text use this).
    pub fn build_from_docs(snapshot: &Snapshot, docs: &[Doc], out_dir: &Path, opts: &BuildOptions) -> Result<BuildReport> {
        let t_all = Instant::now();
        let hash = snapshot.content_hash_hex();
        let final_dir = out_dir.join(&hash[..16]);
        let tmp = out_dir.join(format!("{}.tmp-{}", &hash[..16], std::process::id()));
        if tmp.exists() {
            std::fs::remove_dir_all(&tmp).map_err(io(&tmp))?;
        }
        std::fs::create_dir_all(&tmp).map_err(io(&tmp))?;
        let n = snapshot.num_nodes() as usize;
        let threads = threads_of(opts.threads);
        let mut phases = BTreeMap::new();
        let mut bytes = BTreeMap::new();
        let mut timed = |name: &str, t: Instant| {
            phases.insert(name.to_string(), t.elapsed().as_secs_f64());
        };

        let t = Instant::now();
        bytes.insert("texts".into(), textstore::write(docs, n, &tmp.join("texts.bin"))?);
        timed("texts", t);

        let t = Instant::now();
        lexical::build(docs, &tmp.join("lexical"), threads)?;
        bytes.insert("lexical".into(), dir_len(&tmp.join("lexical")));
        timed("lexical", t);

        let t = Instant::now();
        let fdocs: Vec<(NodeIx, Vec<&str>)> = docs
            .iter()
            .map(|d| (d.node, d.formula_sources()))
            .filter(|(_, s)| !s.is_empty())
            .collect();
        let fstats = formula::build(&fdocs, n, &tmp.join("formula.bin"))?;
        bytes.insert("formula".into(), fstats.bytes);
        timed("formula", t);

        let t = Instant::now();
        let enc_texts: Vec<String> = docs.iter().map(encoder::doc_text).collect();
        let enc = choose_encoder(opts, &enc_texts, &tmp)?;
        drop(enc_texts);
        timed("encoder_load", t);
        let t = Instant::now();
        let cache_path = out_dir.join(format!("emb-{}.cache", sanitize(&enc.name())));
        let (codes, encoded, cache_hits) = embed(&enc, docs, &cache_path)?;
        let dim = codes.first().map_or(0, |c| c.0.len());
        timed("embed", t);
        let t = Instant::now();
        let nodes: Vec<NodeIx> = docs.iter().map(|d| d.node).collect();
        bytes.insert("vectors".into(), dense::write(&nodes, &codes, n, dim, &tmp.join("vectors.bin"))?);
        drop(codes);
        timed("vectors", t);
        if opts.hnsw {
            let t = Instant::now();
            let v = dense::Vectors::open(&tmp.join("vectors.bin"))?;
            bytes.insert("hnsw".into(), dense::Hnsw::build(&v, &tmp.join("hnsw.usearch"), threads)?);
            timed("hnsw", t);
        }
        let mut report = BuildReport {
            index_dir: final_dir.clone(),
            documents: docs.len() as u64,
            secs: 0.0,
            phases,
            encoder: enc.name(),
            dim,
            encoded,
            cache_hits,
            formula: fstats,
            bytes,
        };
        report.secs = t_all.elapsed().as_secs_f64();
        let meta = Meta {
            format: INDEX_FORMAT,
            snapshot_hash: hash.clone(),
            num_nodes: n as u64,
            documents: docs.len() as u64,
            encoder_kind: enc.kind().into(),
            encoder_name: enc.name(),
            dim,
            hnsw: opts.hnsw,
            build: serde_json::to_value(&report).expect("report serializes"),
        };
        drop(enc);
        let mp = tmp.join("meta.json");
        std::fs::write(&mp, serde_json::to_vec_pretty(&meta).expect("meta serializes")).map_err(io(&mp))?;
        if final_dir.exists() {
            std::fs::remove_dir_all(&final_dir).map_err(io(&final_dir))?;
        }
        std::fs::rename(&tmp, &final_dir).map_err(io(&final_dir))?;
        Ok(report)
    }

    /// Open indexes built for `snapshot`: `dir` itself or `dir/<hash16>/`.  Fails if
    /// they belong to another snapshot.  The semantic lane scans exactly unless
    /// `ATLAS_DENSE=hnsw` (see the note above `EncoderChoice`).  A neural encoder that
    /// cannot load disables the semantic lane (reported by `status`), it does not fail the open.
    pub fn open(snapshot: &Snapshot, dir: &Path) -> Result<SearchIndex> {
        let hash = snapshot.content_hash_hex();
        let dir = if dir.join("meta.json").exists() { dir.to_path_buf() } else { dir.join(&hash[..16]) };
        let mp = dir.join("meta.json");
        let bytes = std::fs::read(&mp).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound(format!("search index for snapshot {} in {}", &hash[..16], dir.display())),
            _ => Error::io(&mp, e),
        })?;
        let meta: Meta = serde_json::from_slice(&bytes).map_err(|e| Error::Format(format!("{}: {e}", mp.display())))?;
        if meta.format != INDEX_FORMAT {
            return Err(Error::Format(format!("search index format {} (expected {INDEX_FORMAT})", meta.format)));
        }
        if meta.snapshot_hash != hash || meta.num_nodes != snapshot.num_nodes() as u64 {
            return Err(Error::Integrity(format!(
                "search index in {} was built for snapshot {}, not {}",
                dir.display(),
                &meta.snapshot_hash[..16.min(meta.snapshot_hash.len())],
                &hash[..16]
            )));
        }
        let lexical = lexical::LexicalIndex::open(&dir.join("lexical"))?;
        let formula = formula::FormulaIndex::open(&dir.join("formula.bin"))?;
        let texts = textstore::TextStore::open(&dir.join("texts.bin"))?;
        let vectors = dense::Vectors::open(&dir.join("vectors.bin"))?;
        if [formula.num_nodes(), texts.num_nodes(), vectors.num_nodes()].iter().any(|&x| x != meta.num_nodes as usize) {
            return Err(Error::Integrity(format!("{}: components disagree on the node count", dir.display())));
        }
        let hnsw = if meta.hnsw { Some(dense::Hnsw::open(&dir.join("hnsw.usearch"))?) } else { None };
        let (encoder, encoder_error) = match Self::query_encoder(&meta, &dir) {
            Ok(e) => (Some(e), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let tokenizers = TokenizerManager::default();
        text::register(&tokenizers);
        let use_hnsw = hnsw.is_some() && std::env::var("ATLAS_DENSE").as_deref() == Ok("hnsw");
        let dense_method = if use_hnsw { DenseMethod::Hnsw } else { DenseMethod::Brute };
        let mut concepts = vec![0u64; (snapshot.num_nodes() as usize).div_ceil(64)];
        for &c in snapshot.concepts() {
            concepts[c as usize >> 6] |= 1 << (c & 63);
        }
        let concepts = Arc::new(concepts);
        Ok(SearchIndex { dir, meta, lexical, formula, texts, vectors, hnsw, encoder, encoder_error, dense_method, tokenizers, concepts })
    }

    fn query_encoder(meta: &Meta, dir: &Path) -> Result<Encoder> {
        match meta.encoder_kind.as_str() {
            "lsi" => Ok(Encoder::Lsi(Lsi::load(&dir.join("lsi.bin"))?)),
            #[cfg(feature = "neural")]
            "neural" => {
                // query encoding: small sessions, several so concurrent queries do not queue
                let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
                let n = encoder::neural::Neural::load(
                    &meta.encoder_name,
                    env("ATLAS_QUERY_SESSIONS", 2),
                    env("ATLAS_QUERY_THREADS", 2),
                )?;
                Ok(Encoder::Neural(n))
            }
            k => Err(Error::NotImplemented(if k == "neural" { "semantic lane: built without `neural`" } else { "semantic lane: unknown encoder" })),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn dense_method(&self) -> DenseMethod {
        self.dense_method
    }

    /// The semantic lane's int8 matrix (benchmarks score HNSW results exactly with it).
    pub fn vectors(&self) -> &dense::Vectors {
        &self.vectors
    }

    /// Switch the semantic lane between HNSW and the exact scan (benchmarks, recall checks).
    pub fn set_dense_method(&mut self, m: DenseMethod) -> Result<()> {
        if m == DenseMethod::Hnsw && self.hnsw.is_none() {
            return Err(Error::NotFound("no HNSW graph in this index".into()));
        }
        self.dense_method = m;
        Ok(())
    }

    pub fn set_hnsw_expansion(&self, ef: usize) {
        if let Some(h) = &self.hnsw {
            h.set_expansion(ef);
        }
    }

    /// For `/api/health`: encoder, documents, lanes available.
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "dir": self.dir,
            "documents": self.meta.documents,
            "encoder": self.meta.encoder_name,
            "dim": self.meta.dim,
            "semantic": self.encoder_error.clone().map_or("ok".to_string(), |e| format!("unavailable: {e}")),
            "dense_method": self.dense_method,
            "formula_documents": self.formula.num_docs(),
        })
    }

    pub fn search(&self, snapshot: &Snapshot, query: &SearchQuery) -> Result<SearchResponse> {
        self.search_explained(snapshot, query).map(|(r, _)| r)
    }

    /// Query vector of the semantic lane (benchmarks time it separately).
    pub fn encode_query(&self, text: &str) -> Result<Vec<f32>> {
        self.encoder.as_ref().ok_or_else(|| Error::NotFound("semantic lane unavailable".into()))?.encode_query(text)
    }

    /// Semantic lane alone with a ready query vector.
    pub fn dense_search(&self, qv: &[f32], k: usize, keep: Option<&(dyn Fn(NodeIx) -> bool + Sync)>) -> Result<Vec<(NodeIx, f32)>> {
        match (self.dense_method, &self.hnsw) {
            (DenseMethod::Hnsw, Some(h)) => h.search(qv, k, keep),
            _ => Ok(self.vectors.brute(qv, k, true, keep.unwrap_or(&|_| true))),
        }
    }

    pub fn search_explained(&self, snap: &Snapshot, q: &SearchQuery) -> Result<(SearchResponse, Explain)> {
        let t0 = Instant::now();
        let parsed = router::parse_query(&q.q);
        let text = parsed.text.trim().to_string();
        let mut route = router::route(&text);
        if let Some(only) = parsed.filters.only_lane {
            route.weights = Lane::ALL.map(|l| if lane_name(l) == only { 1.0 } else { 0.0 });
        }
        let limit = q.limit.min(MAX_LIMIT);
        let mut resp = SearchResponse { query: q.q.clone(), lang: q.lang.clone(), limit: q.limit, truncated: false, hits: vec![] };
        let explain = |lanes: Vec<LaneRun>, route: Route| Explain {
            text: text.clone(),
            route,
            filters: parsed.filters.clone(),
            ignored: parsed.ignored.clone(),
            lanes,
            dense_method: self.dense_method,
            micros: t0.elapsed().as_micros() as u64,
        };
        if text.is_empty() || limit == 0 {
            return Ok((resp, explain(Vec::new(), route)));
        }
        let types = TypeMask::new(&q.types, &parsed.filters);
        let assume = AssumeFilter::new(snap, &parsed.filters);
        let k = if assume.is_active() { FILTERED_CANDIDATES } else { (limit * 5).max(100) };
        let keep = |n: NodeIx| types.keep(snap, n);
        let ((lex, con), (form, sem)) = rayon::join(
            || {
                rayon::join(
                    || self.lane_lexical(&text, &q.lang, k, types.bitset(snap), Lane::Lexical, route.weight(Lane::Lexical)),
                    || {
                        let allow = and_bits(&self.concepts, types.bitset(snap));
                        self.lane_lexical(&text, &q.lang, k, Some(allow), Lane::Concept, route.weight(Lane::Concept))
                    },
                )
            },
            || {
                rayon::join(
                    || self.lane_formula(&text, k, &keep, route.weight(Lane::Formula)),
                    || self.lane_semantic(&text, k, &types, &keep, route.weight(Lane::Semantic)),
                )
            },
        );
        let mut runs = [lex?, form, sem?, con?];
        let folded_q = self.fold_join(&text);
        // a label or alias equal to the query; `natural` leaves out Lean names (language `und`)
        let names_query = |n: NodeIx, natural: bool| {
            self.texts.texts(n).iter().any(|t| {
                matches!(t.field, Field::Label | Field::Alias)
                    && !(natural && t.lang == "und")
                    && self.fold_join(&t.text) == folded_q
            })
        };
        // concept lane: only the concepts whose everyday name is the query count
        let (con_hits, con_run) = &mut runs[Lane::Concept as usize];
        con_hits.retain(|&(n, _)| names_query(n, true));
        con_run.candidates = con_hits.len();
        let concept_named = !con_hits.is_empty();
        // weighted RRF
        let mut cands: AHashMap<NodeIx, Cand> = AHashMap::new();
        for (li, (hits, run)) in runs.iter().enumerate() {
            for (rank, &(n, s)) in hits.iter().enumerate() {
                let c = cands.entry(n).or_insert(Cand { score: 0.0, lanes: [None; 4] });
                c.score += run.weight / (RRF_K + rank as f64 + 1.0);
                c.lanes[li] = Some((rank as u32 + 1, s));
            }
        }
        let mut ranked: Vec<(NodeIx, Cand)> = cands.into_iter().filter(|(n, _)| assume.keep(snap, *n)).collect();
        if route.kind != RouteKind::Formula {
            // when a concept bears the query as its everyday name, a Lean short name
            // (`MeasureTheory.integral` has the alias `integral`) is not an exact match
            for (n, c) in ranked.iter_mut() {
                if names_query(*n, concept_named) {
                    c.score += EXACT_BONUS;
                }
            }
        }
        ranked.sort_by(|a, b| b.1.score.total_cmp(&a.1.score).then(a.0.cmp(&b.0)));
        if route.diversify {
            ranked = diversify(snap, ranked, limit);
        }
        resp.truncated = ranked.len() > limit;
        let qtoks: AHashSet<String> = self.fold_tokens(&text).into_iter().collect();
        resp.hits = ranked
            .into_iter()
            .take(limit)
            .map(|(n, c)| {
                let lane = |i: usize| c.lanes[i].map(|(rank, score)| LaneScore { rank, score: score as f64 });
                let lanes = Lanes { lexical: lane(0), formula: lane(1), semantic: lane(2), concept: lane(3) };
                let formula_led = c.lanes[1].is_some_and(|(fr, _)| c.lanes[0].is_none_or(|(lr, _)| fr <= lr));
                Hit {
                    id: snap.node_id(n).to_string(),
                    entity_type: snap.node_type(n),
                    label: snap.label(n, &q.lang).to_string(),
                    matched: self.best_match(snap, n, &qtoks, &q.lang, formula_led),
                    score: c.score,
                    lanes,
                }
            })
            .collect();
        let lanes = runs.into_iter().map(|(_, r)| r).collect();
        Ok((resp, explain(lanes, route)))
    }

    /// BM25 lane; `allow` (node bitset) restricts it (type filters; the concept layer for `Lane::Concept`).
    fn lane_lexical(
        &self,
        text: &str,
        lang: &str,
        k: usize,
        allow: Option<Arc<Vec<u64>>>,
        lane: Lane,
        w: f64,
    ) -> Result<(Vec<(NodeIx, f32)>, LaneRun)> {
        let t = Instant::now();
        let mut run = LaneRun { lane, weight: w, candidates: 0, micros: 0, note: None };
        if w == 0.0 {
            return Ok((Vec::new(), run));
        }
        let hits = match self.lexical.parse(text, lang) {
            None => {
                run.note = Some("no searchable term".into());
                Vec::new()
            }
            Some(lq) => self.lexical.search(&lq, k, allow)?,
        };
        run.candidates = hits.len();
        run.micros = t.elapsed().as_micros() as u64;
        Ok((hits, run))
    }

    fn lane_formula(&self, text: &str, k: usize, keep: &(dyn Fn(NodeIx) -> bool + Sync), w: f64) -> (Vec<(NodeIx, f32)>, LaneRun) {
        let t = Instant::now();
        let mut run = LaneRun { lane: Lane::Formula, weight: w, candidates: 0, micros: 0, note: None };
        if w == 0.0 {
            return (Vec::new(), run);
        }
        let trees = formula::trees_of(text);
        if trees.is_empty() {
            run.note = Some("no formula parsed".into());
        }
        let hits = self.formula.search(&trees, k, keep);
        run.candidates = hits.len();
        run.micros = t.elapsed().as_micros() as u64;
        (hits, run)
    }

    fn lane_semantic(
        &self,
        text: &str,
        k: usize,
        types: &TypeMask,
        keep: &(dyn Fn(NodeIx) -> bool + Sync),
        w: f64,
    ) -> Result<(Vec<(NodeIx, f32)>, LaneRun)> {
        let t = Instant::now();
        let mut run = LaneRun { lane: Lane::Semantic, weight: w, candidates: 0, micros: 0, note: None };
        if w == 0.0 {
            return Ok((Vec::new(), run));
        }
        let Some(enc) = &self.encoder else {
            run.note = self.encoder_error.clone();
            return Ok((Vec::new(), run));
        };
        let qv = enc.encode_query(text)?;
        let enc_us = t.elapsed().as_micros();
        let hits = self.dense_search(&qv, k, types.is_active().then_some(keep))?;
        run.note = Some(format!("encode {enc_us} us"));
        run.candidates = hits.len();
        run.micros = t.elapsed().as_micros() as u64;
        Ok((hits, run))
    }

    fn fold_tokens(&self, s: &str) -> Vec<String> {
        match self.tokenizers.get(text::FOLD) {
            Some(mut a) => text::analyze(&mut a, s),
            None => Vec::new(),
        }
    }

    fn fold_join(&self, s: &str) -> String {
        self.fold_tokens(s).join(" ")
    }

    /// The text of `n` that best explains the hit: most query words (labels and
    /// aliases first, the query language first); the formula when the formula lane led.
    fn best_match(&self, snap: &Snapshot, n: NodeIx, qtoks: &AHashSet<String>, lang: &str, formula_led: bool) -> Matched {
        let texts = self.texts.texts(n);
        let field_w = |f: Field| match f {
            Field::Label => 3.0,
            Field::Alias | Field::ExternalId => 2.0,
            Field::Formula | Field::Statement if formula_led => 4.0,
            _ => 1.0,
        };
        let best = texts
            .iter()
            .map(|t| {
                let toks: AHashSet<String> = self.fold_tokens(&t.text).into_iter().collect();
                let overlap = toks.iter().filter(|w| qtoks.contains(*w)).count() as f64;
                let formula_bonus = if formula_led && matches!(t.field, Field::Formula | Field::Statement) { 1.0 } else { 0.0 };
                let lang_bonus = if t.lang == lang { 0.25 } else { 0.0 };
                ((overlap + formula_bonus) * field_w(t.field) + lang_bonus, t)
            })
            .filter(|(s, _)| *s >= 1.0)
            .max_by(|a, b| a.0.total_cmp(&b.0));
        match best {
            Some((_, t)) => {
                let shown = if t.field == Field::Formula && formula::mathml::is_mathml(&t.text) {
                    formula::mathml::to_latex(&t.text)
                } else {
                    t.text.clone()
                };
                Matched { field: t.field.as_str().into(), lang: t.lang.clone(), text: clip(&shown) }
            }
            None => Matched { field: "label".into(), lang: lang.into(), text: snap.label(n, lang).to_string() },
        }
    }
}

fn lane_name(l: Lane) -> &'static str {
    match l {
        Lane::Lexical => "lexical",
        Lane::Formula => "formula",
        Lane::Semantic => "semantic",
        Lane::Concept => "concept",
    }
}

/// `a` restricted to the nodes of `b` (when there is a `b`).
fn and_bits(a: &Arc<Vec<u64>>, b: Option<Arc<Vec<u64>>>) -> Arc<Vec<u64>> {
    match b {
        None => Arc::clone(a),
        Some(b) => Arc::new(a.iter().zip(b.iter()).map(|(x, y)| x & y).collect()),
    }
}

fn clip(s: &str) -> String {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    match s.char_indices().nth(MAX_MATCHED_CHARS) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s,
    }
}

/// Spread the first `limit` hits over areas: at most `max(2, limit / 4)` per area;
/// the overflow keeps its order after them.  Hits without an area are not capped.
fn diversify(snap: &Snapshot, ranked: Vec<(NodeIx, Cand)>, limit: usize) -> Vec<(NodeIx, Cand)> {
    let cap = (limit / 4).max(2);
    let mut per: AHashMap<NodeIx, usize> = AHashMap::new();
    let (mut head, mut rest) = (Vec::with_capacity(limit), Vec::new());
    for (n, c) in ranked {
        let fits = head.len() < limit
            && snap.area(n).is_none_or(|a| {
                let k = per.entry(a).or_insert(0);
                *k += 1;
                *k <= cap
            });
        if fits { head.push((n, c)) } else { rest.push((n, c)) }
    }
    head.extend(rest);
    head
}

/// Allowed entity types (`types` of the query plus `type:` / `-type:`).
struct TypeMask {
    allow: [bool; EntityType::COUNT],
    active: bool,
}

impl TypeMask {
    fn new(types: &[EntityType], f: &Filters) -> TypeMask {
        let wanted: Vec<EntityType> = types.iter().chain(&f.types).copied().collect();
        let mut allow = [wanted.is_empty(); EntityType::COUNT];
        for t in wanted {
            allow[t as usize] = true;
        }
        for t in &f.not_types {
            allow[*t as usize] = false;
        }
        TypeMask { allow, active: allow.iter().any(|a| !a) }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    fn keep(&self, snap: &Snapshot, n: NodeIx) -> bool {
        !self.active || self.allow[snap.node_types()[n as usize] as usize]
    }

    /// Node bitset for tantivy's filter collector (`None` when no type filter).
    /// ponytail: rebuilt per query (one pass over the node types, ~1 ms per million
    /// nodes); cache per type set if filtered queries dominate.
    fn bitset(&self, snap: &Snapshot) -> Option<Arc<Vec<u64>>> {
        if !self.active {
            return None;
        }
        let types = snap.node_types();
        let mut bits = vec![0u64; types.len().div_ceil(64)];
        for (i, &t) in types.iter().enumerate() {
            if self.allow[t as usize] {
                bits[i >> 6] |= 1 << (i & 63);
            }
        }
        Some(Arc::new(bits))
    }
}

/// `assume:` / `-assume:`: an entity id (matched against assumption participants of
/// the hit's claims) or text (matched in their free-text assumptions and validity domains).
struct AssumeFilter {
    want: Vec<(Option<NodeIx>, String)>,
    avoid: Vec<(Option<NodeIx>, String)>,
}

fn fold_simple(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' => 'a',
            'é' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'ï' => 'i',
            'ó' | 'ò' | 'ö' => 'o',
            'ú' | 'ù' | 'ü' => 'u',
            '_' => ' ',
            c => c,
        })
        .collect()
}

impl AssumeFilter {
    fn new(snap: &Snapshot, f: &Filters) -> AssumeFilter {
        let needle = |v: &String| (snap.resolve(v), fold_simple(v));
        AssumeFilter { want: f.assume.iter().map(needle).collect(), avoid: f.not_assume.iter().map(needle).collect() }
    }

    fn is_active(&self) -> bool {
        !self.want.is_empty() || !self.avoid.is_empty()
    }

    fn matches(snap: &Snapshot, n: NodeIx, (node, text): &(Option<NodeIx>, String)) -> bool {
        let mut seen = AHashSet::new();
        for &c in snap.out_edges(n).claim {
            if !seen.insert(c) || snap.claim_subject(c) != n {
                continue;
            }
            if node.is_some_and(|a| snap.claim_context(c).any(|(r, m)| r == Role::Assumption && m == a)) {
                return true;
            }
            let free = snap.claim_assumptions(c).chain(snap.claim_validity_domain(c));
            if !text.is_empty() && free.into_iter().any(|s| fold_simple(s).contains(text.as_str())) {
                return true;
            }
        }
        false
    }

    fn keep(&self, snap: &Snapshot, n: NodeIx) -> bool {
        self.want.iter().all(|w| Self::matches(snap, n, w)) && !self.avoid.iter().any(|w| Self::matches(snap, n, w))
    }
}

#[cfg(test)]
mod tests;
