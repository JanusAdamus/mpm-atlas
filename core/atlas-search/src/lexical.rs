//! Lexical lane: tantivy BM25 over labels, aliases, external ids, descriptions and
//! statements, each analysed three ways (`text.rs`): Spanish stems, English stems
//! and identifiers (accent-folded words plus Lean/Mathlib names split on dots,
//! underscores and CamelCase).
//!
//! Index-side routing by language: `es` text -> `_es` + `_id`, `en` -> `_en` + `_id`,
//! anything else (`und`, Lean code, other languages) -> all three.  Query side: for
//! each kind the three analyser variants are a DisMax (so a text indexed three times
//! is not counted three times), and kinds add up with the weights of `Store.search`
//! (label 3, alias 2, external id 2, description 1, statement 1).  The query
//! language's stemmed field gets full weight, the other one `OTHER_LANG`.  The last
//! query word is also a prefix on labels and aliases (as `Store.search`), and a
//! multi-word query adds an exact-phrase bonus on labels.
//!
//! Measured alternative, rejected (2026-10-17, 1M synthetic snapshot, 200 queries per
//! class): a flat disjunction of boosted term queries (block-max WAND eligible) plus a
//! separate prefix/phrase query was slower than this DisMax query: lexical p50 43 vs
//! 26 ms for named objects, 117 vs 19 ms for natural language (Zipf-frequent terms
//! leave WAND little to skip, and two queries do the work twice).

use crate::corpus::{Doc, Field};
use crate::text;
use atlas_core::{Error, NodeIx, Result};
use rayon::prelude::*;
use std::path::Path;
use std::sync::Arc;
use tantivy::collector::{FilterCollector, TopDocs};
use tantivy::columnar::Column;
use tantivy::query::{BooleanQuery, BoostQuery, DisjunctionMaxQuery, Occur, PhrasePrefixQuery, PhraseQuery, Query, TermQuery};
use tantivy::schema::{
    FAST, Field as TField, IndexRecordOption, NumericOptions, Schema, TextFieldIndexing, TextOptions,
};
use tantivy::tokenizer::TextAnalyzer;
use tantivy::{Index, IndexReader, ReloadPolicy, TantivyDocument, Term};

/// Kinds indexed by the lexical lane and their `Store.search` weights.
pub const KINDS: [(Field, f32); 5] = [
    (Field::Label, 3.0),
    (Field::Alias, 2.0),
    (Field::ExternalId, 2.0),
    (Field::Description, 1.0),
    (Field::Statement, 1.0),
];
const OTHER_LANG: f32 = 0.6;
const DISMAX_TIE: f32 = 0.1;
const PREFIX_BOOST: f32 = 0.5;
const PHRASE_BOOST: f32 = 2.0;
/// Bonus for a description or statement holding every query word (the implicit AND of
/// FTS5).  Without it, docstring-fragment queries lost to label hits on one frequent word:
/// AtlasBench v3 dev, en/text nDCG@10 0.635 hybrid vs 0.883 FTS5 baseline.
const ALL_WORDS_BOOST: f32 = 2.0;
const ALL_WORDS_MIN: usize = 3;
const PREFIX_EXPANSIONS: u32 = 64;
const WRITER_HEAP: usize = 1 << 30;
const MAX_QUERY_TERMS: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Es,
    En,
    Id,
}

impl Variant {
    const ALL: [Variant; 3] = [Variant::Es, Variant::En, Variant::Id];
    fn analyzer(self) -> &'static str {
        match self {
            Variant::Es => text::ES,
            Variant::En => text::EN,
            Variant::Id => text::LEAN,
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            Variant::Es => "es",
            Variant::En => "en",
            Variant::Id => "id",
        }
    }
}

struct Fields {
    node: TField,
    /// `[kind][variant]`
    text: Vec<[TField; 3]>,
}

fn schema() -> (Schema, Fields) {
    let mut b = Schema::builder();
    let node = b.add_u64_field("node", NumericOptions::default() | FAST);
    let text = KINDS
        .iter()
        .map(|(k, _)| {
            Variant::ALL.map(|v| {
                let idx = TextFieldIndexing::default()
                    .set_tokenizer(v.analyzer())
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions);
                b.add_text_field(&format!("{}_{}", k.as_str(), v.suffix()), TextOptions::default().set_indexing_options(idx))
            })
        })
        .collect();
    (b.build(), Fields { node, text })
}

fn fields_of(schema: &Schema) -> Result<Fields> {
    let get = |name: &str| schema.get_field(name).map_err(|e| Error::Format(format!("lexical schema: {e}")));
    let node = get("node")?;
    let mut text = Vec::new();
    for (k, _) in KINDS {
        let mut row = [node; 3];
        for (i, v) in Variant::ALL.iter().enumerate() {
            row[i] = get(&format!("{}_{}", k.as_str(), v.suffix()))?;
        }
        text.push(row);
    }
    Ok(Fields { node, text })
}

fn tv(e: tantivy::TantivyError) -> Error {
    Error::Format(format!("tantivy: {e}"))
}

fn variants_for(lang: &str) -> &'static [Variant] {
    match lang {
        "es" => &[Variant::Es, Variant::Id],
        "en" => &[Variant::En, Variant::Id],
        _ => &Variant::ALL,
    }
}

/// Build the tantivy index in `dir` (created) with `threads` indexing threads.
pub fn build(docs: &[Doc], dir: &Path, threads: usize) -> Result<u64> {
    std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let (schema, f) = schema();
    let index = Index::create_in_dir(dir, schema).map_err(tv)?;
    text::register(index.tokenizers());
    let threads = threads.clamp(1, 16);
    let mut writer = index.writer_with_num_threads::<TantivyDocument>(threads, WRITER_HEAP.max(threads * (64 << 20))).map_err(tv)?;
    // documents are assembled in parallel; tantivy's writer threads do the analysis
    let batch = 50_000;
    for chunk in docs.chunks(batch) {
        let built: Vec<TantivyDocument> = chunk
            .par_iter()
            .map(|d| {
                let mut doc = TantivyDocument::default();
                doc.add_u64(f.node, d.node as u64);
                for (ki, (kind, _)) in KINDS.iter().enumerate() {
                    for t in d.of(*kind) {
                        for v in variants_for(&t.lang) {
                            doc.add_text(f.text[ki][*v as usize], &t.text);
                        }
                    }
                }
                doc
            })
            .collect();
        for doc in built {
            writer.add_document(doc).map_err(tv)?;
        }
    }
    writer.commit().map_err(tv)?;
    writer.wait_merging_threads().map_err(tv)?;
    Ok(docs.len() as u64)
}

pub struct LexicalIndex {
    index: Index,
    reader: IndexReader,
    f: Fields,
    /// node column per segment ordinal of the (never reloaded) searcher
    nodes: Vec<Column<u64>>,
}

/// The analysed query, built once and reused by every kind.
pub struct LexQuery {
    query: Box<dyn Query>,
    pub terms: usize,
}

#[cfg(test)]
impl LexQuery {
    /// Number of conjunctive (all words required) clauses, for tests.
    pub(crate) fn must_clauses(&self) -> usize {
        format!("{:?}", self.query).matches("Must").count()
    }
}

impl LexicalIndex {
    pub fn open(dir: &Path) -> Result<LexicalIndex> {
        let index = Index::open_in_dir(dir).map_err(tv)?;
        text::register(index.tokenizers());
        let f = fields_of(&index.schema())?;
        let reader = index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().map_err(tv)?;
        let searcher = reader.searcher();
        let nodes = searcher
            .segment_readers()
            .iter()
            .map(|s| s.fast_fields().u64("node"))
            .collect::<tantivy::Result<Vec<_>>>()
            .map_err(tv)?;
        Ok(LexicalIndex { index, reader, f, nodes })
    }

    pub fn num_docs(&self) -> u64 {
        self.reader.searcher().num_docs()
    }

    fn analyze(&self, analyzer: &str, q: &str) -> Vec<String> {
        let Some(mut a): Option<TextAnalyzer> = self.index.tokenizers().get(analyzer) else { return Vec::new() };
        let mut v = text::analyze(&mut a, q);
        v.truncate(MAX_QUERY_TERMS);
        v
    }

    /// Analyse `q` for query language `lang`.  `None` if no term survives analysis.
    pub fn parse(&self, q: &str, lang: &str) -> Option<LexQuery> {
        let terms: [Vec<String>; 3] = Variant::ALL.map(|v| self.analyze(v.analyzer(), q));
        if terms.iter().all(Vec::is_empty) {
            return None;
        }
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();
        for (ki, (kind, kw)) in KINDS.iter().enumerate() {
            let mut variants: Vec<Box<dyn Query>> = Vec::new();
            for v in Variant::ALL {
                if *kind == Field::ExternalId && v != Variant::Id {
                    continue;
                }
                let field = self.f.text[ki][v as usize];
                let should: Vec<(Occur, Box<dyn Query>)> = terms[v as usize]
                    .iter()
                    .map(|t| {
                        let q: Box<dyn Query> =
                            Box::new(TermQuery::new(Term::from_field_text(field, t), IndexRecordOption::WithFreqs));
                        (Occur::Should, q)
                    })
                    .collect();
                if should.is_empty() {
                    continue;
                }
                let w = match (v, lang) {
                    (Variant::Es, "es") | (Variant::En, "en") | (Variant::Id, _) => 1.0,
                    _ => OTHER_LANG,
                };
                variants.push(Box::new(BoostQuery::new(Box::new(BooleanQuery::new(should)), w)));
            }
            if !variants.is_empty() {
                let dm = DisjunctionMaxQuery::with_tie_breaker(variants, DISMAX_TIE);
                clauses.push((Occur::Should, Box::new(BoostQuery::new(Box::new(dm), *kw))));
            }
        }
        let id_terms = &terms[Variant::Id as usize];
        // last word as a prefix on labels and aliases (constant score)
        if let Some(last) = id_terms.last().filter(|t| t.chars().count() >= 2 && !q.ends_with(char::is_whitespace)) {
            for kind_ix in [0usize, 1] {
                let field = self.f.text[kind_ix][Variant::Id as usize];
                let mut p = PhrasePrefixQuery::new(vec![Term::from_field_text(field, last)]);
                p.set_max_expansions(PREFIX_EXPANSIONS);
                clauses.push((Occur::Should, Box::new(BoostQuery::new(Box::new(p), PREFIX_BOOST))));
            }
        }
        // exact phrase on labels
        let words: Vec<&String> = id_terms.iter().collect();
        if words.len() >= 2 {
            let field = self.f.text[0][Variant::Id as usize];
            let phrase = PhraseQuery::new(words.iter().map(|w| Term::from_field_text(field, w)).collect());
            clauses.push((Occur::Should, Box::new(BoostQuery::new(Box::new(phrase), PHRASE_BOOST))));
        }
        // every query word in one description or statement (conjunctive, so cheap to evaluate)
        if words.len() >= ALL_WORDS_MIN {
            for kind in [Field::Description, Field::Statement] {
                let Some(ki) = KINDS.iter().position(|(k, _)| *k == kind) else { continue };
                let field = self.f.text[ki][Variant::Id as usize];
                let must: Vec<(Occur, Box<dyn Query>)> = words
                    .iter()
                    .map(|w| {
                        let q: Box<dyn Query> =
                            Box::new(TermQuery::new(Term::from_field_text(field, w), IndexRecordOption::WithFreqs));
                        (Occur::Must, q)
                    })
                    .collect();
                clauses.push((Occur::Should, Box::new(BoostQuery::new(Box::new(BooleanQuery::new(must)), ALL_WORDS_BOOST))));
            }
        }
        let terms = terms.iter().map(Vec::len).max().unwrap_or(0);
        Some(LexQuery { query: Box::new(BooleanQuery::new(clauses)), terms })
    }

    /// Top `k` nodes for `q`, best first.  `allow` (a node bitset) filters inside the collector.
    pub fn search(&self, q: &LexQuery, k: usize, allow: Option<Arc<Vec<u64>>>) -> Result<Vec<(NodeIx, f32)>> {
        if k == 0 {
            return Ok(Vec::new());
        }
        let searcher = self.reader.searcher();
        let top = TopDocs::with_limit(k).order_by_score();
        let docs = match allow {
            None => searcher.search(q.query.as_ref(), &top).map_err(tv)?,
            Some(bits) => {
                let pred = move |n: u64| bits.get((n >> 6) as usize).is_some_and(|w| w >> (n & 63) & 1 == 1);
                searcher.search(q.query.as_ref(), &FilterCollector::new("node".to_string(), pred, top)).map_err(tv)?
            }
        };
        Ok(docs
            .into_iter()
            .filter_map(|(score, addr)| {
                let col = self.nodes.get(addr.segment_ord as usize)?;
                col.first(addr.doc_id).map(|n| (n as NodeIx, score))
            })
            .collect())
    }
}
