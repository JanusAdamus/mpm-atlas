//! Works fragment (ADR-0005 §8.4): a lexical-only tantivy index over the works layer snapshot.
//! One document per work or witness: its title(s), the names of its authors (as aliases) and its
//! external ids (DOI, zbMATH, OpenAlex, CDLI…).  No dense vectors: 7 M works × 384 dims would not fit
//! the server's memory budget, and the cold encode would take about 17 h (ADR §8.4).

use crate::corpus::{self, Doc, Field, Text};
use crate::lexical::{self, LexicalIndex};
use atlas_core::{Dir, EntityType, NodeIx, RelationType, Result, Snapshot};
use serde::Serialize;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Serialize)]
pub struct WorksBuildReport {
    pub docs: u64,
    pub load_secs: f64,
    pub index_secs: f64,
    pub bytes: u64,
}

fn is_work(t: EntityType) -> bool {
    matches!(t, EntityType::Publication | EntityType::Witness)
}

/// Documents of the works of `layer`: registry texts plus author names from `authored_by`.
pub fn docs(layer: &Snapshot, db: &Path) -> Result<Vec<Doc>> {
    let mut docs = corpus::load(layer, db)?;
    docs.retain(|d| is_work(layer.node_type(d.node)));
    let out = layer.csr(Dir::Out);
    for d in &mut docs {
        for e in out.edges_rel(d.node, RelationType::AuthoredBy).iter() {
            d.texts.push(Text { field: Field::Alias, lang: "und".into(), text: layer.label(e.nbr, "en").to_string() });
        }
    }
    Ok(docs)
}

pub fn build(layer: &Snapshot, db: &Path, dir: &Path, threads: usize) -> Result<WorksBuildReport> {
    let t = Instant::now();
    let docs = docs(layer, db)?;
    let load_secs = t.elapsed().as_secs_f64();
    let t = Instant::now();
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(|e| atlas_core::Error::io(dir, e))?;
    }
    let n = lexical::build(&docs, dir, threads)?;
    let bytes = dir_bytes(dir);
    Ok(WorksBuildReport { docs: n, load_secs, index_secs: t.elapsed().as_secs_f64(), bytes })
}

fn dir_bytes(dir: &Path) -> u64 {
    std::fs::read_dir(dir).map_or(0, |rd| rd.flatten().filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum())
}

pub struct WorksIndex {
    lex: LexicalIndex,
}

impl WorksIndex {
    pub fn open(dir: &Path) -> Result<WorksIndex> {
        Ok(WorksIndex { lex: LexicalIndex::open(dir)? })
    }

    pub fn num_docs(&self) -> u64 {
        self.lex.num_docs()
    }

    /// Top `k` works for `q` (BM25, ES/EN/identifier analysers), best first.
    pub fn search(&self, q: &str, lang: &str, k: usize) -> Result<Vec<(NodeIx, f32)>> {
        match self.lex.parse(q, lang) {
            Some(lq) => self.lex.search(&lq, k, None),
            None => Ok(Vec::new()),
        }
    }
}
