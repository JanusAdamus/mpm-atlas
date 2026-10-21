//! SQLite registry -> snapshot.  Parallel readers, in-memory assembly, one write.
//!
//! Consistency: every registry write appends to the event log, so `MAX(events.seq)`
//! is read before and after; if it moved, the store changed during the scan and
//! the compile is retried (each reader is its own connection, so they cannot
//! share one SQLite read transaction).

use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::concepts;
use crate::error::{Error, Result};
use crate::format::{NONE, SUPPORTED_SCHEMA};
use crate::hashidx;
use crate::layer::{self, Scope};
use crate::read::{self, ClaimChunk, EntChunk, Lookup, PartChunk, ProvChunk};
use crate::strings::StrTable;
use crate::vocab::{EvidenceState, RelationType, Role, scope_bit};
use crate::write::{self, Columns};

const RETRIES: usize = 3;

#[derive(Clone, Debug)]
pub struct CompileOptions {
    /// Registry (`data/atlas.db`).
    pub db: PathBuf,
    /// Output directory (`data/snapshot/`); receives `atlas-<hash16>.snap` and `CURRENT`.
    pub out_dir: PathBuf,
    /// Worker threads; 0 = all cores.
    pub threads: usize,
    /// Rowid ranges per thread for each table scan.
    pub chunks_per_thread: usize,
    /// `Some(core hash)` writes a layer snapshot (works, ADR-0005 §8.4) bound to that core snapshot;
    /// `Snapshot::open_layer` rejects it next to any other core.  None = core snapshot.
    pub core_content_hash: Option<[u8; 32]>,
}

impl CompileOptions {
    pub fn new(db: impl Into<PathBuf>, out_dir: impl Into<PathBuf>) -> Self {
        CompileOptions { db: db.into(), out_dir: out_dir.into(), threads: 0, chunks_per_thread: 4, core_content_hash: None }
    }
}

/// Relations a bibliographic source may state (ADR-0005 §4, layer 2); shared with Python.
const SOURCE_POLICY: &str = include_str!("../../../shapes/source_relations.json");

#[derive(Clone, Debug, Serialize)]
pub struct Phase {
    pub name: &'static str,
    pub secs: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompileReport {
    pub snapshot: PathBuf,
    pub content_hash: String,
    pub bytes: u64,
    pub schema_version: u32,
    pub source_event_seq: u64,
    pub nodes: u32,
    pub claims: u32,
    pub edges: u32,
    pub groups: u32,
    pub sources: u32,
    pub provenance_rows: u32,
    pub strings: u32,
    /// Concept layer: concepts, declarations with a primary concept, aggregated concept -> concept edges.
    pub concepts: u32,
    pub declarations_with_concept: u32,
    pub concept_edges: u32,
    pub threads: usize,
    pub phases: Vec<Phase>,
    pub total_secs: f64,
}

struct Timer {
    t: Instant,
    phases: Vec<Phase>,
}

impl Timer {
    fn lap(&mut self, name: &'static str) {
        let now = Instant::now();
        self.phases.push(Phase { name, secs: (now - self.t).as_secs_f64() });
        self.t = now;
    }
}

/// Compile `opts.db` into a new snapshot in `opts.out_dir` and point `CURRENT` at it.
pub fn compile(opts: &CompileOptions) -> Result<CompileReport> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(opts.threads)
        .build()
        .map_err(|e| Error::Registry(format!("thread pool: {e}")))?;
    pool.install(|| {
        for attempt in 1..=RETRIES {
            match compile_once(opts) {
                Err(Error::Registry(m)) if m.starts_with("registry changed") && attempt < RETRIES => continue,
                other => return other,
            }
        }
        unreachable!("loop returns on the last attempt")
    })
}

fn event_seq(conn: &rusqlite::Connection) -> Result<u64> {
    Ok(conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |r| r.get::<_, i64>(0))? as u64)
}

fn compile_once(opts: &CompileOptions) -> Result<CompileReport> {
    let start = Instant::now();
    let mut tm = Timer { t: start, phases: Vec::new() };
    if !opts.db.is_file() {
        return Err(Error::NotFound(format!("registry {}", opts.db.display())));
    }
    let conn = read::open_ro(&opts.db)?;
    let schema: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if schema == 0 || schema > SUPPORTED_SCHEMA {
        return Err(Error::Registry(format!("registry schema v{schema}; this compiler reads v1..=v{SUPPORTED_SCHEMA}")));
    }
    let seq0 = event_seq(&conn)?;
    let chunks = rayon::current_num_threads() * opts.chunks_per_thread.max(1);
    let db = opts.db.as_path();

    let sources = read::read_sources(&conn)?;
    let scope = if opts.core_content_hash.is_some() { Scope::Works } else { Scope::Core };
    let (ent_filter, claim_filter, part_filter, prov_filter) = (
        layer::entity_filter(scope)?,
        layer::claim_filter(scope, "id")?,
        layer::claim_filter(scope, "claim_id")?,
        layer::claim_filter(scope, "claim_id")?,
    );
    let mut ents: Vec<EntChunk> = read::rowid_ranges(&conn, "entities", chunks)?
        .par_iter()
        .map(|&r| read::read_entities(db, r, &ent_filter))
        .collect::<Result<_>>()?;
    if scope == Scope::Works {
        ents.push(read::read_entities_by_id(db, &layer::stub_ids(&conn)?)?);
    }
    tm.lap("read_entities");

    let mut cols = Columns::default();
    let n_nodes: usize = ents.iter().map(|c| c.types.len()).sum();
    check_u32(n_nodes, "entities")?;
    let mut st = StrTable::with_capacity(3 * n_nodes, 48 * n_nodes);
    for s in &sources {
        cols.source_id.push(st.push(&s.id)?);
        cols.source_title.push(st.push(&s.title)?);
        cols.source_license.push(st.intern(&s.license)?);
    }
    let merged = append_entities(&mut cols, &mut st, &ents)?;
    drop(ents);
    cols.node_hash = hashidx::build(n_nodes, |i| st.get(cols.node_id[i as usize]));
    cols.node_canon = {
        let node_of = |id: &str| hashidx::lookup(&cols.node_hash, id, |i| st.get(cols.node_id[i as usize]));
        resolve_merges(n_nodes, &merged, &node_of)?
    };
    cols.merged_nodes = (0..n_nodes as u32).filter(|&i| cols.node_canon[i as usize] != i).collect();
    cols.core_content_hash = opts.core_content_hash.unwrap_or([0; 32]);
    {
        let mut curation = read::read_curation(&conn, schema)?;
        let (hash, ids, canon) = (&cols.node_hash, &cols.node_id, &cols.node_canon);
        let node_of = |id: &str| hashidx::lookup(hash, id, |i| st.get(ids[i as usize])).map(|n| canon[n as usize]);
        if scope == Scope::Works {
            curation.retain(|r| node_of(&r.target).is_some()); // the core applies the rest
        } else {
            // a works id is curated by the layer compile, never a missing core target
            let mut keep = Vec::with_capacity(curation.len());
            for r in curation {
                if !layer::is_works_id(&r.target)? {
                    keep.push(r);
                }
            }
            curation = keep;
        }
        let found: Vec<Option<u32>> = curation.iter().map(|r| node_of(&r.target)).collect();
        (cols.node_flags, cols.node_show_es, cols.node_show_en) = apply_curation(n_nodes, &curation, &found, &mut st)?;
    }
    tm.lap("index_entities");

    let claims: Vec<ClaimChunk> = read::rowid_ranges(&conn, "claims", chunks)?
        .par_iter()
        .map(|&r| read::read_claims(db, r, &claim_filter))
        .collect::<Result<_>>()?;
    tm.lap("read_claims");
    let n_groups = append_claims(&mut cols, &mut st, &claims)?;
    let conf: Vec<f64> = claims.iter().flat_map(|c| c.conf.iter().copied()).collect();
    drop(claims);
    let n_claims = cols.claim_rel.len();
    cols.claim_hash = hashidx::build(n_claims, |i| st.get(cols.claim_id[i as usize]));
    tm.lap("index_claims");

    let (parts, provs) = {
        let node_of = |id: &str| hashidx::lookup(&cols.node_hash, id, |i| st.get(cols.node_id[i as usize]));
        let claim_of = |id: &str| hashidx::lookup(&cols.claim_hash, id, |i| st.get(cols.claim_id[i as usize]));
        let look = Lookup {
            node: &node_of,
            canon: &cols.node_canon,
            claim: &claim_of,
            sources: sources.iter().enumerate().map(|(i, s)| (s.id.clone(), i as u32)).collect(),
        };
        let step = (n_claims / (chunks * 16)).max(1);
        let sample: Vec<&str> = cols.claim_id.iter().step_by(step).map(|&s| st.get(s)).collect();
        let parts: Vec<PartChunk> = read::key_ranges(&sample, chunks)
            .par_iter()
            .map(|r| read::read_participants(db, r, &look, &part_filter))
            .collect::<Result<_>>()?;
        tm.lap("read_participants");
        let provs: Vec<ProvChunk> = read::rowid_ranges(&conn, "provenance", chunks)?
            .par_iter()
            .map(|&r| read::read_provenance(db, r, &look, &prov_filter))
            .collect::<Result<_>>()?;
        (parts, provs)
    };
    tm.lap("read_provenance");
    if event_seq(&conn)? != seq0 {
        return Err(Error::Registry("registry changed during compile".into()));
    }

    scatter_participants(&mut cols, &parts)?;
    drop(parts);
    scatter_provenance(&mut cols, &mut st, &provs)?;
    drop(provs);
    audit_sources(&cols, &st)?;
    build_groups(&mut cols, n_groups);
    tm.lap("assemble_claims");
    node_stats(&mut cols);
    tm.lap("node_stats");
    build_csr(&mut cols)?;
    tm.lap("build_csr");
    cols.strings = st;
    cols.concepts = {
        let csr = concepts::OutCsr {
            off: &cols.out_off,
            nbr: &cols.out_nbr,
            claim: &cols.out_claim,
            rel: &cols.out_rel,
            state: &cols.out_state,
        };
        let (strings, ids) = (&cols.strings, &cols.node_id);
        concepts::build(&csr, &conf, &|n| strings.get(ids[n as usize]))
    };
    drop(conf);
    tm.lap("concepts");

    let written = write::write_snapshot(&opts.out_dir, &cols, schema, seq0)?;
    tm.lap("write");
    Ok(CompileReport {
        snapshot: written.path,
        content_hash: written.hash_hex,
        bytes: written.bytes,
        schema_version: schema,
        source_event_seq: seq0,
        nodes: cols.num_nodes(),
        claims: cols.num_claims(),
        edges: cols.out_nbr.len() as u32,
        groups: n_groups as u32,
        sources: cols.source_id.len() as u32,
        provenance_rows: cols.prov_src.len() as u32,
        strings: cols.strings.len() as u32,
        concepts: cols.concepts.concepts.len() as u32,
        declarations_with_concept: cols.concepts.members.len() as u32,
        concept_edges: cols.concepts.edge_dst.len() as u32,
        threads: rayon::current_num_threads(),
        phases: tm.phases,
        total_secs: start.elapsed().as_secs_f64(),
    })
}

fn check_u32(n: usize, what: &str) -> Result<()> {
    if n >= NONE as usize {
        return Err(Error::Registry(format!("too many {what} for u32 ids (format v1)")));
    }
    Ok(())
}

/// Node columns from the entity chunks; returns (node, id it is merged into).
fn append_entities(cols: &mut Columns, st: &mut StrTable, ents: &[EntChunk]) -> Result<Vec<(u32, String)>> {
    let mut merged = Vec::new();
    for c in ents {
        let node0 = cols.node_id.len() as u32;
        let base = st.append(&c.ids)?;
        cols.node_id.extend((0..c.ids.len() as u32).map(|i| base + i));
        cols.node_type.extend_from_slice(&c.types);
        let lb = st.append(&c.labels)?;
        cols.node_label_es.extend(c.es.iter().map(|&l| if l == NONE { NONE } else { lb + l }));
        cols.node_label_en.extend(c.en.iter().map(|&l| lb + l));
        merged.extend(c.merged.iter().map(|(l, into)| (node0 + l, into.clone())));
    }
    Ok(merged)
}

/// node -> canonical node, following `merged_into` chains.
fn resolve_merges(n: usize, merged: &[(u32, String)], node_of: &dyn Fn(&str) -> Option<u32>) -> Result<Vec<u32>> {
    let mut canon: Vec<u32> = (0..n as u32).collect();
    for (node, into) in merged {
        canon[*node as usize] = node_of(into).ok_or_else(|| Error::Registry(format!("merged into unknown {into}")))?;
    }
    for (node, _) in merged {
        let (mut c, mut steps) = (canon[*node as usize], 0usize);
        while canon[c as usize] != c {
            c = canon[c as usize];
            steps += 1;
            if steps > merged.len() {
                return Err(Error::Registry("merge cycle in entities.merged_into".into()));
            }
        }
        canon[*node as usize] = c;
    }
    Ok(canon)
}

/// Claim columns from the claim chunks; alternative groups get indexes in id order.
fn append_claims(cols: &mut Columns, st: &mut StrTable, claims: &[ClaimChunk]) -> Result<usize> {
    let n: usize = claims.iter().map(|c| c.rel.len()).sum();
    check_u32(n, "claims")?;
    let mut gids: Vec<&str> = claims.iter().flat_map(|c| c.groups.vec.iter()).collect();
    gids.par_sort_unstable();
    gids.dedup();
    check_u32(gids.len(), "groups")?;
    for g in &gids {
        cols.group_id.push(st.push(g)?);
    }
    cols.claim_asm_off.push(0);
    for c in claims {
        let base = st.append(&c.ids)?;
        cols.claim_id.extend((0..c.ids.len() as u32).map(|i| base + i));
        cols.claim_rel.extend_from_slice(&c.rel);
        cols.claim_state.extend_from_slice(&c.state);
        let gmap: Vec<u32> = c.groups.vec.iter().map(|g| gids.binary_search(&g).expect("collected") as u32).collect();
        cols.claim_group.extend(c.group.iter().map(|&g| if g == NONE { NONE } else { gmap[g as usize] }));
        let tmap: Vec<u32> = c.texts.vec.iter().map(|t| st.intern(t)).collect::<Result<_>>()?;
        let asm0 = cols.claim_asm.len() as u32;
        cols.claim_asm_off.extend(c.asm_off[1..].iter().map(|&o| o + asm0));
        cols.claim_asm.extend(c.asm.iter().map(|&t| tmap[t as usize]));
        cols.claim_validity.extend(c.validity.iter().map(|&t| if t == NONE { NONE } else { tmap[t as usize] }));
    }
    Ok(gids.len())
}

/// Participant rows -> subject column, target CSR (object or AND-set of premises)
/// and context CSR, keeping each claim's position order.
fn scatter_participants(cols: &mut Columns, parts: &[PartChunk]) -> Result<()> {
    let n = cols.claim_rel.len();
    let mut subject = vec![NONE; n];
    let (mut tgt_off, mut ctx_off) = (vec![0u32; n + 1], vec![0u32; n + 1]);
    for p in parts {
        for i in 0..p.claim.len() {
            let c = p.claim[i] as usize;
            match Role::from_code(p.role[i]) {
                Some(Role::Subject) if subject[c] == NONE => subject[c] = p.node[i],
                Some(Role::Subject) => return Err(Error::Registry(format!("claim #{c} has two subjects"))),
                Some(Role::Object | Role::Premise) => tgt_off[c + 1] += 1,
                _ => ctx_off[c + 1] += 1,
            }
        }
    }
    for c in 0..n {
        if subject[c] == NONE || tgt_off[c + 1] == 0 {
            return Err(Error::Registry(format!("claim #{c} lacks a subject or a target")));
        }
        tgt_off[c + 1] += tgt_off[c];
        ctx_off[c + 1] += ctx_off[c];
    }
    check_u32(tgt_off[n] as usize + ctx_off[n] as usize, "claim participants")?;
    let mut tgt = vec![0u32; tgt_off[n] as usize];
    let mut ctx_node = vec![0u32; ctx_off[n] as usize];
    let mut ctx_role = vec![0u8; ctx_off[n] as usize];
    let (mut tf, mut cf) = (tgt_off.clone(), ctx_off.clone());
    for p in parts {
        for i in 0..p.claim.len() {
            let c = p.claim[i] as usize;
            match Role::from_code(p.role[i]) {
                Some(Role::Subject) => {}
                Some(Role::Object | Role::Premise) => {
                    tgt[tf[c] as usize] = p.node[i];
                    tf[c] += 1;
                }
                _ => {
                    ctx_node[cf[c] as usize] = p.node[i];
                    ctx_role[cf[c] as usize] = p.role[i];
                    cf[c] += 1;
                }
            }
        }
    }
    (cols.claim_subject, cols.claim_tgt_off, cols.claim_tgt) = (subject, tgt_off, tgt);
    (cols.claim_ctx_off, cols.claim_ctx_node, cols.claim_ctx_role) = (ctx_off, ctx_node, ctx_role);
    Ok(())
}

/// Provenance rows -> per-claim CSR in rowid order; locators and licenses interned.
fn scatter_provenance(cols: &mut Columns, st: &mut StrTable, provs: &[ProvChunk]) -> Result<()> {
    let n = cols.claim_rel.len();
    let mut off = vec![0u32; n + 1];
    for p in provs {
        for &c in &p.claim {
            off[c as usize + 1] += 1;
        }
    }
    for c in 0..n {
        off[c + 1] += off[c];
    }
    let total = off[n] as usize;
    check_u32(total, "provenance rows")?;
    let (mut src, mut loc, mut lic) = (vec![0u32; total], vec![0u32; total], vec![0u32; total]);
    let mut fill = off.clone();
    for p in provs {
        let tmap: Vec<u32> = p.texts.vec.iter().map(|t| st.intern(t)).collect::<Result<_>>()?;
        for i in 0..p.claim.len() {
            let at = fill[p.claim[i] as usize] as usize;
            (src[at], loc[at], lic[at]) = (p.src[i], tmap[p.loc[i] as usize], tmap[p.lic[i] as usize]);
            fill[p.claim[i] as usize] += 1;
        }
    }
    (cols.prov_off, cols.prov_src, cols.prov_locator, cols.prov_license) = (off, src, loc, lic);
    Ok(())
}

type Presentation = (Vec<u8>, Vec<u32>, Vec<u32>);

/// NodeFlags and presentation labels from the active curation rows (`found[i]`: canonical node of
/// row i).  Later rows win for a label; undone rows were filtered out by the reader.
fn apply_curation(n: usize, rows: &[read::CurationRow], found: &[Option<u32>], st: &mut StrTable) -> Result<Presentation> {
    let (mut flags, mut es, mut en) = (vec![0u8; n], vec![NONE; n], vec![NONE; n]);
    for (r, node) in rows.iter().zip(found) {
        let bad = |m: &str| Error::Registry(format!("curation {} on {}: {m}", r.id, r.target));
        let node = node.ok_or_else(|| bad("unknown target entity"))? as usize;
        if r.kind == "exclude_presentation" {
            let scopes = r.payload["scopes"].as_array().ok_or_else(|| bad("payload without scopes"))?;
            for s in scopes {
                flags[node] |= s.as_str().and_then(scope_bit).ok_or_else(|| bad("unknown presentation scope"))?;
            }
            continue;
        }
        let text = r.payload["text"].as_str().ok_or_else(|| bad("presentation_label without text"))?;
        match r.payload["lang"].as_str() {
            Some("es") => es[node] = st.intern(text)?,
            Some("en") => en[node] = st.intern(text)?,
            _ => {} // the snapshot only carries es/en labels
        }
    }
    Ok((flags, es, en))
}

#[derive(serde::Deserialize)]
struct SourcePolicy {
    bibliographic_sources: BiblioPolicy,
}

#[derive(serde::Deserialize)]
struct BiblioPolicy {
    prefixes: Vec<String>,
    allowed_relations: Vec<RelationType>,
}

/// Compile-time audit (ADR-0005 §4): a current claim sourced from a bibliographic source must use an
/// allowed relation.  Fails the compile with the first offending claim.
fn audit_sources(cols: &Columns, st: &StrTable) -> Result<()> {
    let policy: SourcePolicy = serde_json::from_str(SOURCE_POLICY)
        .map_err(|e| Error::Registry(format!("shapes/source_relations.json: {e}")))?;
    let p = policy.bibliographic_sources;
    let allowed: u32 = p.allowed_relations.iter().map(|r| r.bit()).sum();
    let restricted: Vec<bool> =
        cols.source_id.iter().map(|&s| p.prefixes.iter().any(|pre| st.get(s).starts_with(pre.as_str()))).collect();
    if !restricted.contains(&true) {
        return Ok(());
    }
    let bad = (0..cols.claim_rel.len()).into_par_iter().find_first(|&c| {
        let rel = RelationType::from_code(cols.claim_rel[c]).expect("relation code");
        let current = EvidenceState::from_code(cols.claim_state[c]).is_some_and(|s| s.is_current());
        let rows = cols.prov_off[c] as usize..cols.prov_off[c + 1] as usize;
        current && rel.bit() & allowed == 0 && cols.prov_src[rows].iter().any(|&s| restricted[s as usize])
    });
    match bad {
        None => Ok(()),
        Some(c) => Err(Error::Registry(format!(
            "source audit: claim {} ({}) is sourced from a bibliographic source, which may only state {:?} \
             (shapes/source_relations.json)",
            st.get(cols.claim_id[c]),
            RelationType::from_code(cols.claim_rel[c]).expect("relation code"),
            p.allowed_relations.iter().map(|r| r.as_str()).collect::<Vec<_>>()
        ))),
    }
}

fn build_groups(cols: &mut Columns, n_groups: usize) {
    let mut count = vec![0u32; n_groups + 1];
    for &g in &cols.claim_group {
        if g != NONE {
            count[g as usize + 1] += 1;
        }
    }
    for i in 0..n_groups {
        count[i + 1] += count[i];
    }
    let mut fill = count.clone();
    let mut members = vec![0u32; count[n_groups] as usize];
    for (c, &g) in cols.claim_group.iter().enumerate() {
        if g != NONE {
            members[fill[g as usize] as usize] = c as u32;
            fill[g as usize] += 1;
        }
    }
    cols.group_off = count;
    cols.group_claims = members;
}

/// claim count, evidence-state mask and area per node (claims of any state count
/// for coverage, like `Store.coverage`; area comes from current claims only).
fn node_stats(cols: &mut Columns) {
    let n = cols.node_type.len();
    let (mut count, mut mask, mut area) = (vec![0u32; n], vec![0u8; n], vec![NONE; n]);
    let mut seen: Vec<u32> = Vec::with_capacity(16);
    for c in 0..cols.claim_rel.len() {
        let state = cols.claim_state[c];
        seen.clear();
        seen.push(cols.claim_subject[c]);
        let t = cols.claim_tgt_off[c] as usize..cols.claim_tgt_off[c + 1] as usize;
        seen.extend_from_slice(&cols.claim_tgt[t.clone()]);
        let x = cols.claim_ctx_off[c] as usize..cols.claim_ctx_off[c + 1] as usize;
        seen.extend_from_slice(&cols.claim_ctx_node[x]);
        seen.sort_unstable();
        seen.dedup();
        for &v in &seen {
            count[v as usize] += 1;
            mask[v as usize] |= 1 << state;
        }
        let subject = cols.claim_subject[c] as usize;
        let current = EvidenceState::from_code(state).is_some_and(|s| s.is_current());
        if cols.claim_rel[c] == RelationType::PartOfArea.code() && current && area[subject] == NONE {
            area[subject] = cols.claim_tgt[t.start];
        }
    }
    (cols.node_claim_count, cols.node_state_mask, cols.node_area) = (count, mask, area);
}

#[derive(Clone, Copy)]
struct E {
    node: u32,
    rel: u8,
    state: u8,
    nbr: u32,
    claim: u32,
}

/// Typed CSR per direction over current claims (one edge per target; a conjunction
/// yields one edge per premise, all sharing the claim id).
fn build_csr(cols: &mut Columns) -> Result<()> {
    let n = cols.node_type.len();
    let edges = |out: bool| -> Vec<E> {
        (0..cols.claim_rel.len())
            .into_par_iter()
            .filter(|&c| EvidenceState::from_code(cols.claim_state[c]).is_some_and(|s| s.is_current()))
            .flat_map_iter(|c| {
                let (s, rel, state) = (cols.claim_subject[c], cols.claim_rel[c], cols.claim_state[c]);
                let t = cols.claim_tgt_off[c] as usize..cols.claim_tgt_off[c + 1] as usize;
                cols.claim_tgt[t].iter().map(move |&t| {
                    let (node, nbr) = if out { (s, t) } else { (t, s) };
                    E { node, rel, state, nbr, claim: c as u32 }
                })
            })
            .collect()
    };
    for out in [true, false] {
        let mut e = edges(out);
        check_u32(e.len(), "edges")?;
        e.par_sort_unstable_by_key(|e| (e.node, e.rel, e.nbr, e.claim));
        let mut off = vec![0u32; n + 1];
        for x in &e {
            off[x.node as usize + 1] += 1;
        }
        for i in 0..n {
            off[i + 1] += off[i];
        }
        let nbr = e.par_iter().map(|x| x.nbr).collect();
        let claim = e.par_iter().map(|x| x.claim).collect();
        let rel = e.par_iter().map(|x| x.rel).collect();
        let state = e.par_iter().map(|x| x.state).collect();
        if out {
            (cols.out_off, cols.out_nbr, cols.out_claim, cols.out_rel, cols.out_state) = (off, nbr, claim, rel, state);
        } else {
            (cols.in_off, cols.in_nbr, cols.in_claim, cols.in_rel, cols.in_state) = (off, nbr, claim, rel, state);
        }
    }
    Ok(())
}

/// `dir/CURRENT` names the live snapshot file.
pub fn current_snapshot(dir: &Path) -> Result<PathBuf> {
    let cur = dir.join("CURRENT");
    let name = std::fs::read_to_string(&cur).map_err(|e| Error::io(&cur, e))?;
    Ok(dir.join(name.trim()))
}
