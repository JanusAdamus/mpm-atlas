//! Searchable text of every snapshot node, read from the registry.
//!
//! The snapshot keeps only ids, types and labels; aliases, descriptions, statements,
//! formulas and external ids live in `entities.data` (JSON written by
//! `Entity.to_json`).  Rows are read by rowid ranges on parallel read-only
//! connections.  A merged entity's texts are added to its survivor's document
//! (merges are redirects), and entities newer than the snapshot are skipped: the
//! snapshot defines what is served.

use atlas_core::{EntityType, Error, NodeIx, Result, Snapshot};
use rayon::prelude::*;
use rusqlite::{Connection, OpenFlags, params};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// Where a text came from; `as_str` is the `matched.field` of `/api/search`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Field {
    Label,
    Alias,
    ExternalId,
    Description,
    Statement,
    Formula,
}

impl Field {
    pub const ALL: [Field; 6] =
        [Field::Label, Field::Alias, Field::ExternalId, Field::Description, Field::Statement, Field::Formula];

    pub fn as_str(self) -> &'static str {
        match self {
            Field::Label => "label",
            Field::Alias => "alias",
            Field::ExternalId => "external_id",
            Field::Description => "description",
            Field::Statement => "statement",
            Field::Formula => "formula",
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(c: u8) -> Option<Field> {
        Field::ALL.get(c as usize).copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub field: Field,
    pub lang: String,
    pub text: String,
}

/// All texts of one node (its own entity plus every entity merged into it).
#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub node: NodeIx,
    pub texts: Vec<Text>,
}

impl Doc {
    pub fn of(&self, field: Field) -> impl Iterator<Item = &Text> {
        self.texts.iter().filter(move |t| t.field == field)
    }

    /// Formula sources: the `formula` field and formal statements (Lean declarations,
    /// LaTeX).  Prose statements are left to the lexical and semantic lanes.
    pub fn formula_sources(&self) -> Vec<&str> {
        self.texts
            .iter()
            .filter(|t| t.field == Field::Formula || t.field == Field::Statement && crate::formula::looks_formal(&t.text))
            .map(|t| t.text.as_str())
            .collect()
    }
}

const MMAP_BYTES: i64 = 1 << 40;
const CHUNKS_PER_THREAD: usize = 4;

pub(crate) fn open_ro(db: &Path) -> Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(db, flags)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "cache_size", -65536)?;
    conn.pragma_update(None, "mmap_size", MMAP_BYTES)?;
    Ok(conn)
}

#[derive(Deserialize)]
struct EntityJson {
    #[serde(default)]
    labels: BTreeMap<String, String>,
    #[serde(default)]
    aliases: Vec<(String, String)>,
    #[serde(default)]
    descriptions: BTreeMap<String, String>,
    #[serde(default)]
    statements: BTreeMap<String, String>,
    #[serde(default)]
    formula: Option<String>,
    #[serde(default)]
    external_ids: Vec<(String, String)>,
}

impl EntityJson {
    fn texts(self) -> Vec<Text> {
        let t = |field, lang: String, text: String| Text { field, lang, text };
        let mut out = Vec::new();
        out.extend(self.labels.into_iter().map(|(l, x)| t(Field::Label, l, x)));
        out.extend(self.aliases.into_iter().map(|(l, x)| t(Field::Alias, l, x)));
        out.extend(self.descriptions.into_iter().map(|(l, x)| t(Field::Description, l, x)));
        out.extend(self.statements.into_iter().map(|(l, x)| t(Field::Statement, l, x)));
        out.extend(self.formula.into_iter().map(|x| t(Field::Formula, "und".into(), x)));
        out.extend(self.external_ids.into_iter().map(|(s, v)| t(Field::ExternalId, "und".into(), format!("{s}:{v}"))));
        out.retain(|x| !x.text.trim().is_empty());
        out
    }
}

fn rowid_ranges(conn: &Connection, n: usize) -> Result<Vec<(i64, i64)>> {
    let (lo, hi): (Option<i64>, Option<i64>) =
        conn.query_row("SELECT MIN(rowid), MAX(rowid) FROM entities", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let (Some(lo), Some(hi)) = (lo, hi) else { return Ok(Vec::new()) };
    let span = (hi - lo + 1) as u64;
    let n = (n.max(1) as u64).min(span);
    let step = span.div_ceil(n) as i64;
    Ok((0..n as i64).map(|i| (lo + i * step, (lo + (i + 1) * step - 1).min(hi))).filter(|(a, b)| a <= b).collect())
}

fn read_range(db: &Path, snap: &Snapshot, (lo, hi): (i64, i64)) -> Result<Vec<(NodeIx, Vec<Text>)>> {
    let conn = open_ro(db)?;
    let mut st = conn.prepare("SELECT id, data FROM entities WHERE rowid BETWEEN ?1 AND ?2 ORDER BY rowid")?;
    let mut rows = st.query(params![lo, hi])?;
    let mut out = Vec::new();
    while let Some(r) = rows.next()? {
        let id: String = r.get(0)?;
        let Some(node) = snap.resolve(&id) else { continue };
        if snap.node_type(node) == EntityType::Note {
            continue; // a ficha sentence is panel text, never a hit (ADR-0005 §6)
        }
        let data = r.get_ref(1)?.as_str()?;
        let e: EntityJson = serde_json::from_str(data).map_err(|e| Error::Registry(format!("entity {id}: {e}")))?;
        out.push((node, e.texts()));
    }
    Ok(out)
}

/// One document per snapshot node that has any text, sorted by node.
pub fn load(snap: &Snapshot, db: &Path) -> Result<Vec<Doc>> {
    if !db.exists() {
        return Err(Error::NotFound(format!("registry {}", db.display())));
    }
    let conn = open_ro(db)?;
    let ranges = rowid_ranges(&conn, rayon::current_num_threads() * CHUNKS_PER_THREAD)?;
    let chunks: Vec<Vec<(NodeIx, Vec<Text>)>> =
        ranges.par_iter().map(|&r| read_range(db, snap, r)).collect::<Result<_>>()?;
    let mut slots: Vec<Vec<Text>> = vec![Vec::new(); snap.num_nodes() as usize];
    for (node, texts) in chunks.into_iter().flatten() {
        slots[node as usize].extend(texts);
    }
    Ok(slots
        .into_iter()
        .enumerate()
        .filter(|(_, t)| !t.is_empty())
        .map(|(n, texts)| Doc { node: n as NodeIx, texts })
        .collect())
}
