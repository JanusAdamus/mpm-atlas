//! Parallel, streaming readers over the SQLite registry (schema v1).
//!
//! Each reader takes a rowid range and opens its own read-only connection, so
//! rayon can scan disjoint ranges of one table at once.  Rows come back in rowid
//! order; chunk results are concatenated in order, so output is deterministic.

use ahash::RandomState;
use rusqlite::{Connection, OpenFlags, params};
use serde::de::{Deserializer, MapAccess, Visitor};
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::error::{Error, Result};
use crate::format::NONE;
use crate::strings::{DedupStrVec, StrVec};
use crate::vocab::{EntityType, EvidenceState, RelationType, Role};

/// Map up to 1 TiB of the registry (needs SQLITE_MAX_MMAP_SIZE raised in core/.cargo/config.toml;
/// otherwise SQLite clamps it to 2 GiB).  Parallel readers scale with mmap, not with ReadFile.
const MMAP_BYTES: i64 = 1 << 40;

pub(crate) fn open_ro(db: &Path) -> Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(db, flags)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "cache_size", -65536)?; // 64 MiB per reader
    conn.pragma_update(None, "mmap_size", MMAP_BYTES)?;
    Ok(conn)
}

/// Split `[MIN(rowid), MAX(rowid)]` of `table` into at most `n` inclusive ranges.
pub(crate) fn rowid_ranges(conn: &Connection, table: &str, n: usize) -> Result<Vec<(i64, i64)>> {
    let (lo, hi): (Option<i64>, Option<i64>) =
        conn.query_row(&format!("SELECT MIN(rowid), MAX(rowid) FROM {table}"), [], |r| Ok((r.get(0)?, r.get(1)?)))?;
    let (Some(lo), Some(hi)) = (lo, hi) else { return Ok(Vec::new()) };
    let span = (hi - lo + 1) as u64;
    let n = (n.max(1) as u64).min(span);
    let step = span.div_ceil(n) as i64;
    Ok((0..n as i64).map(|i| (lo + i * step, (lo + (i + 1) * step - 1).min(hi))).filter(|(a, b)| a <= b).collect())
}

pub(crate) struct SourceRow {
    pub id: String,
    pub title: String,
    pub license: String,
}

pub(crate) fn read_sources(conn: &Connection) -> Result<Vec<SourceRow>> {
    #[derive(serde::Deserialize)]
    struct Data {
        title: String,
        license: String,
    }
    let mut st = conn.prepare("SELECT id, data FROM sources ORDER BY id")?;
    let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    rows.map(|row| {
        let (id, data) = row?;
        let d: Data = serde_json::from_str(&data).map_err(|e| Error::Registry(format!("source {id}: {e}")))?;
        Ok(SourceRow { id, title: d.title, license: d.license })
    })
    .collect()
}

// ------------------------------------------------------------------ entities

#[derive(Default)]
pub(crate) struct EntChunk {
    pub ids: StrVec,
    pub types: Vec<u8>,
    /// (local index, id it is merged into)
    pub merged: Vec<(u32, String)>,
    pub labels: StrVec,
    pub es: Vec<u32>,
    pub en: Vec<u32>,
}

/// The two label slots of a node: `es`, and `en` falling back to the first label
/// (so `label(lang)` reproduces `Entity.label`: es -> en -> first).
struct Labels {
    es: Option<String>,
    en_or_first: Option<String>,
}

impl<'de> serde::Deserialize<'de> for Labels {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Labels;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a {lang: text} object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> std::result::Result<Labels, A::Error> {
                let (mut es, mut en, mut first) = (None, None, None);
                while let Some((k, v)) = m.next_entry::<String, String>()? {
                    if first.is_none() {
                        first = Some(v.clone());
                    }
                    match k.as_str() {
                        "es" => es = Some(v),
                        "en" => en = Some(v),
                        _ => {}
                    }
                }
                Ok(Labels { es, en_or_first: en.or(first) })
            }
        }
        d.deserialize_map(V)
    }
}

#[derive(serde::Deserialize)]
struct EntData {
    labels: Labels,
}

pub(crate) fn read_entities(db: &Path, (lo, hi): (i64, i64)) -> Result<EntChunk> {
    let conn = open_ro(db)?;
    let mut st =
        conn.prepare("SELECT id, type, merged_into, data FROM entities WHERE rowid BETWEEN ?1 AND ?2 ORDER BY rowid")?;
    let mut rows = st.query(params![lo, hi])?;
    let mut out = EntChunk::default();
    while let Some(r) = rows.next()? {
        let id = r.get_ref(0)?.as_str()?;
        let ty: EntityType = r.get_ref(1)?.as_str()?.parse().map_err(|e| Error::Registry(format!("{id}: {e}")))?;
        let local = out.ids.push(id);
        out.types.push(ty.code());
        if let Some(m) = r.get_ref(2)?.as_str_or_null()? {
            out.merged.push((local, m.to_owned()));
        }
        let data: EntData =
            serde_json::from_str(r.get_ref(3)?.as_str()?).map_err(|e| Error::Registry(format!("entity {id}: {e}")))?;
        let es = data.labels.es.map_or(NONE, |s| out.labels.push(&s));
        let en = data.labels.en_or_first.ok_or_else(|| Error::Registry(format!("entity {id} has no label")))?;
        out.es.push(es);
        let en = out.labels.push(&en);
        out.en.push(en);
    }
    Ok(out)
}

// ------------------------------------------------------------------ claims

/// Rows of the `claims` table for one rowid range; strings in `groups` / `texts`
/// are local to the chunk.
#[derive(Default)]
pub(crate) struct ClaimChunk {
    pub ids: StrVec,
    pub rel: Vec<u8>,
    pub state: Vec<u8>,
    pub group: Vec<u32>,
    pub groups: DedupStrVec,
    pub asm_off: Vec<u32>,
    pub asm: Vec<u32>,
    pub validity: Vec<u32>,
    pub texts: DedupStrVec,
}

#[derive(serde::Deserialize)]
struct ClaimData {
    #[serde(default)]
    assumptions: Vec<String>,
    #[serde(default)]
    validity_domain: Option<String>,
}

pub(crate) fn read_claims(db: &Path, (lo, hi): (i64, i64)) -> Result<ClaimChunk> {
    let conn = open_ro(db)?;
    let mut st = conn.prepare(
        "SELECT id, relation, evidence_state, alternative_group, data FROM claims \
         WHERE rowid BETWEEN ?1 AND ?2 ORDER BY rowid",
    )?;
    let mut rows = st.query(params![lo, hi])?;
    let mut out = ClaimChunk::default();
    out.asm_off.push(0);
    while let Some(r) = rows.next()? {
        let id = r.get_ref(0)?.as_str()?;
        out.ids.push(id);
        let bad = |e: crate::vocab::UnknownVariant| Error::Registry(format!("claim {id}: {e}"));
        out.rel.push(r.get_ref(1)?.as_str()?.parse::<RelationType>().map_err(bad)?.code());
        out.state.push(r.get_ref(2)?.as_str()?.parse::<EvidenceState>().map_err(bad)?.code());
        out.group.push(r.get_ref(3)?.as_str_or_null()?.map_or(NONE, |g| out.groups.intern(g)));
        let data: ClaimData = serde_json::from_str(r.get_ref(4)?.as_str()?)
            .map_err(|e| Error::Registry(format!("claim {id}: {e}")))?;
        for a in &data.assumptions {
            let t = out.texts.intern(a);
            out.asm.push(t);
        }
        out.asm_off.push(out.asm.len() as u32);
        out.validity.push(data.validity_domain.map_or(NONE, |v| out.texts.intern(&v)));
    }
    Ok(out)
}

/// Resolves registry ids to snapshot indexes during the scans (hash tables over
/// the already-collected id strings; the same tables go into the snapshot).
pub(crate) struct Lookup<'a> {
    pub node: &'a (dyn Fn(&str) -> Option<u32> + Sync),
    pub canon: &'a [u32],
    pub claim: &'a (dyn Fn(&str) -> Option<u32> + Sync),
    pub sources: HashMap<String, u32, RandomState>,
}

impl Lookup<'_> {
    fn node(&self, id: &str, claim: &str) -> Result<u32> {
        (self.node)(id)
            .map(|n| self.canon[n as usize])
            .ok_or_else(|| Error::Registry(format!("claim {claim} refers to unknown entity {id}")))
    }

    fn claim(&self, id: &str) -> Result<u32> {
        (self.claim)(id).ok_or_else(|| Error::Registry(format!("row refers to unknown claim {id}")))
    }
}

/// Participants of a claim-id key range, in (claim_id, position) order.
#[derive(Default)]
pub(crate) struct PartChunk {
    pub claim: Vec<u32>,
    pub role: Vec<u8>,
    pub node: Vec<u32>,
}

/// `[lo, hi)` bounds on `claim_participants.claim_id` (None = open end).
pub(crate) type KeyRange = (Option<String>, Option<String>);

/// Split the claim-id key space into about `n` ranges from a sample of `ids`.
pub(crate) fn key_ranges(ids: &[&str], n: usize) -> Vec<KeyRange> {
    let step = (ids.len() / (n.max(1) * 16)).max(1);
    let mut sample: Vec<&str> = ids.iter().step_by(step).copied().collect();
    sample.sort_unstable();
    let cuts: Vec<String> = (1..n.max(1)).filter_map(|k| sample.get(k * sample.len() / n).map(|s| s.to_string())).collect();
    let mut bounds: Vec<Option<String>> = vec![None];
    bounds.extend(cuts.into_iter().map(Some));
    bounds.dedup();
    bounds.push(None);
    bounds.windows(2).map(|w| (w[0].clone(), w[1].clone())).collect()
}

/// Sequential scan of the participant PK (no per-claim seeks).
pub(crate) fn read_participants(db: &Path, range: &KeyRange, look: &Lookup<'_>) -> Result<PartChunk> {
    let conn = open_ro(db)?;
    let mut sql = String::from("SELECT claim_id, role, entity_id FROM claim_participants");
    match range {
        (None, None) => {}
        (Some(_), None) => sql.push_str(" WHERE claim_id >= ?1"),
        (None, Some(_)) => sql.push_str(" WHERE claim_id < ?1"),
        (Some(_), Some(_)) => sql.push_str(" WHERE claim_id >= ?1 AND claim_id < ?2"),
    }
    sql.push_str(" ORDER BY claim_id, position");
    let mut st = conn.prepare(&sql)?;
    let args: Vec<&str> = [&range.0, &range.1].into_iter().flatten().map(|s| s.as_str()).collect();
    let mut rows = st.query(rusqlite::params_from_iter(args))?;
    let mut out = PartChunk::default();
    let (mut last_id, mut last_ix) = (String::new(), NONE);
    while let Some(r) = rows.next()? {
        let cid = r.get_ref(0)?.as_str()?;
        if cid != last_id {
            last_ix = look.claim(cid)?;
            last_id.clear();
            last_id.push_str(cid);
        }
        let role: Role = r.get_ref(1)?.as_str()?.parse().map_err(|e| Error::Registry(format!("claim {cid}: {e}")))?;
        out.claim.push(last_ix);
        out.role.push(role.code());
        out.node.push(look.node(r.get_ref(2)?.as_str()?, cid)?);
    }
    Ok(out)
}

/// Claim provenance rows of one rowid range, in rowid order.
#[derive(Default)]
pub(crate) struct ProvChunk {
    pub claim: Vec<u32>,
    pub src: Vec<u32>,
    pub loc: Vec<u32>,
    pub lic: Vec<u32>,
    pub texts: DedupStrVec,
}

pub(crate) fn read_provenance(db: &Path, (lo, hi): (i64, i64), look: &Lookup<'_>) -> Result<ProvChunk> {
    let conn = open_ro(db)?;
    let mut st = conn.prepare(
        "SELECT claim_id, source_id, locator, license FROM provenance \
         WHERE rowid BETWEEN ?1 AND ?2 AND claim_id IS NOT NULL ORDER BY rowid",
    )?;
    let mut rows = st.query(params![lo, hi])?;
    let mut out = ProvChunk::default();
    while let Some(r) = rows.next()? {
        let cid = r.get_ref(0)?.as_str()?;
        out.claim.push(look.claim(cid)?);
        let sid = r.get_ref(1)?.as_str()?;
        let s = *look.sources.get(sid).ok_or_else(|| Error::Registry(format!("claim {cid} cites unregistered source {sid}")))?;
        out.src.push(s);
        out.loc.push(out.texts.intern(r.get_ref(2)?.as_str()?));
        out.lic.push(out.texts.intern(r.get_ref(3)?.as_str()?));
    }
    Ok(out)
}
