//! Synthetic registry for benchmarks (not for tests of meaning).
//!
//! Fills an *empty* registry whose schema was created by `atlas.store.Store`
//! (the contract), writing rows directly for speed.  Rows respect the relation
//! endpoint types, conjunction shape and alternative-group rules, ids look like
//! `stable_id` hashes (random b-tree order, as in real ingest) and targets follow
//! a skewed (power-law-like) degree distribution.  It does not write FTS rows or
//! per-row events, so it is only fit for snapshot/graph benchmarks.

use rusqlite::{Connection, params};
use serde::Serialize;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use crate::error::{Error, Result};
use crate::vocab::{EntityType as T, EvidenceState as S, RelationType as R};

#[derive(Clone, Debug)]
pub struct SynthConfig {
    pub entities: u32,
    /// Stop once the current claims have this many (subject, target) edges.
    pub edges: u64,
    pub seed: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct SynthReport {
    pub entities: u64,
    pub claims: u64,
    pub participants: u64,
    pub current_edges: u64,
    pub conjunctions: u64,
    pub alternative_groups: u64,
    pub secs: f64,
}

const TS: &str = "2026-10-16T00:00:00+00:00";
const SOURCES: [(&str, &str); 3] = [("src:synth-a", "CC0-1.0"), ("src:synth-b", "CC-BY-4.0"), ("src:synth-c", "Apache-2.0")];
const ASSUMPTIONS: [&str; 4] = ["finite groups", "char K = 0", "Euclidean plane", "ZFC"];

fn mix(mut z: u64) -> u64 {
    // splitmix64 finalizer: a bijection, so distinct inputs give distinct ids
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(1);
        mix(self.0)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Skewed index: low indices are hubs.
    fn skewed(&mut self, n: usize) -> usize {
        ((n as f64) * self.unit().powi(3)) as usize % n
    }
}

/// (type, weight per mille, es word, en word)
const TYPES: [(T, u32, &str, &str); 10] = [
    (T::Area, 2, "Área", "Area"),
    (T::Person, 20, "Persona", "Person"),
    (T::Publication, 50, "Publicación", "Publication"),
    (T::Application, 10, "Aplicación", "Application"),
    (T::Concept, 220, "Concepto", "Concept"),
    (T::Definition, 100, "Definición", "Definition"),
    (T::Statement, 450, "Teorema", "Theorem"),
    (T::Proof, 100, "Demostración", "Proof"),
    (T::Conjecture, 20, "Conjetura", "Conjecture"),
    (T::Algorithm, 28, "Algoritmo", "Algorithm"),
];

#[derive(Clone, Copy)]
enum Pool {
    Math,
    Of(T),
    Proven,
}

/// (relation, weight per mille, subject pool, object pool)
const RELS: [(R, u32, Pool, Pool); 13] = [
    (R::DependsOnLogically, 300, Pool::Math, Pool::Math),
    (R::PrerequisiteFor, 150, Pool::Math, Pool::Math),
    (R::Generalizes, 100, Pool::Math, Pool::Math),
    (R::SpecialCaseOf, 50, Pool::Math, Pool::Math),
    (R::EquivalentTo, 20, Pool::Math, Pool::Math),
    (R::AnalogousTo, 20, Pool::Math, Pool::Math),
    (R::Proves, 60, Pool::Of(T::Proof), Pool::Proven),
    (R::PartOfArea, 100, Pool::Math, Pool::Of(T::Area)),
    (R::AppearsIn, 80, Pool::Math, Pool::Of(T::Publication)),
    (R::Cites, 60, Pool::Of(T::Publication), Pool::Of(T::Publication)),
    (R::AuthoredBy, 30, Pool::Of(T::Publication), Pool::Of(T::Person)),
    (R::AppliedIn, 20, Pool::Math, Pool::Of(T::Application)),
    (R::NamedAfter, 10, Pool::Math, Pool::Of(T::Person)),
];

struct Pools {
    by_type: Vec<Vec<u32>>,
    math: Vec<u32>,
    proven: Vec<u32>,
}

impl Pools {
    fn get(&self, p: Pool) -> &[u32] {
        match p {
            Pool::Math => &self.math,
            Pool::Of(t) => &self.by_type[t as usize],
            Pool::Proven => &self.proven,
        }
    }
}

fn pick_weighted<X: Copy>(rng: &mut Rng, items: &[(X, u32)]) -> X {
    let total: u32 = items.iter().map(|x| x.1).sum();
    let mut r = (rng.next() % total as u64) as u32;
    for &(x, w) in items {
        if r < w {
            return x;
        }
        r -= w;
    }
    items[items.len() - 1].0
}

fn entity_id(i: u32, seed: u64) -> String {
    format!("e:{:016x}", mix(i as u64 ^ seed))
}

fn claim_id(i: u64, seed: u64) -> String {
    format!("c:{:016x}", mix(i ^ seed.rotate_left(17)))
}

/// Fill the empty registry at `db` (schema created by `atlas.store.Store`).
pub fn generate(db: &Path, cfg: &SynthConfig) -> Result<SynthReport> {
    let t0 = Instant::now();
    let mut conn = Connection::open(db)?;
    let schema: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let existing: i64 = conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
    if schema != 1 || existing != 0 {
        return Err(Error::Registry(format!("synth needs an empty v1 registry (schema v{schema}, {existing} entities)")));
    }
    conn.execute_batch(
        "PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA cache_size=-2000000; \
         PRAGMA temp_store=MEMORY; PRAGMA foreign_keys=OFF; PRAGMA locking_mode=EXCLUSIVE;",
    )?;
    // Secondary indexes are rebuilt from the registry's own DDL afterwards (faster than
    // maintaining them row by row); PRIMARY KEY indexes stay.
    let indexes: Vec<(String, String)> = conn
        .prepare("SELECT name, sql FROM sqlite_master WHERE type='index' AND sql IS NOT NULL")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    for (name, _) in &indexes {
        conn.execute_batch(&format!("DROP INDEX \"{name}\""))?;
    }
    let tx = conn.transaction()?;
    let mut rng = Rng(cfg.seed);
    for (id, lic) in SOURCES {
        let data = format!(r#"{{"id":"{id}","title":"Synthetic benchmark source {id}","license":"{lic}"}}"#);
        tx.execute("INSERT INTO sources VALUES (?1, ?2, ?3)", params![id, data, TS])?;
        tx.execute(
            "INSERT INTO events(ts, actor, kind, target_id, payload) VALUES (?1, 'synth', 'source_added', ?2, ?3)",
            params![TS, id, format!(r#"{{"license":"{lic}"}}"#)],
        )?;
    }
    let pools = insert_entities(&tx, cfg, &mut rng)?;
    let mut report = insert_claims(&tx, cfg, &mut rng, &pools)?;
    tx.commit()?;
    for (_, sql) in &indexes {
        conn.execute_batch(sql)?;
    }
    conn.execute_batch("PRAGMA locking_mode=NORMAL; PRAGMA journal_mode=WAL;")?;
    report.entities = cfg.entities as u64;
    report.secs = t0.elapsed().as_secs_f64();
    Ok(report)
}

fn insert_entities(tx: &rusqlite::Transaction<'_>, cfg: &SynthConfig, rng: &mut Rng) -> Result<Pools> {
    let mut st = tx.prepare("INSERT INTO entities(id, type, data, created_at) VALUES (?1, ?2, ?3, ?4)")?;
    let weights: Vec<(usize, u32)> = TYPES.iter().enumerate().map(|(i, t)| (i, t.1)).collect();
    let mut pools = Pools { by_type: vec![Vec::new(); T::COUNT], math: Vec::new(), proven: Vec::new() };
    let mut data = String::with_capacity(256);
    for i in 0..cfg.entities {
        // guarantee every pool is non-empty
        let k = if (i as usize) < TYPES.len() { i as usize } else { pick_weighted(rng, &weights) };
        let (ty, _, es, en) = TYPES[k];
        let id = entity_id(i, cfg.seed);
        data.clear();
        let _ = write!(data, r#"{{"id":"{id}","type":"{}","labels":{{"#, ty.as_str());
        if rng.below(20) != 0 {
            let _ = write!(data, r#""es":"{es} {i}","#);
        }
        let _ = write!(data, r#""en":"{en} {i}"}}}}"#);
        st.execute(params![id, ty.as_str(), data, TS])?;
        pools.by_type[ty as usize].push(i);
        match ty {
            T::Concept | T::Definition | T::Statement | T::Proof | T::Conjecture | T::Algorithm => pools.math.push(i),
            _ => {}
        }
        if matches!(ty, T::Statement | T::Conjecture) {
            pools.proven.push(i);
        }
    }
    Ok(pools)
}

struct Draft {
    rel: R,
    subject: u32,
    targets: Vec<u32>,
    premises: bool,
    context: Vec<(&'static str, u32)>,
    group: Option<String>,
}

fn draft(rng: &mut Rng, pools: &Pools) -> Draft {
    let weights: Vec<(usize, u32)> = RELS.iter().enumerate().map(|(i, r)| (i, r.1)).collect();
    let (rel, _, sp, op) = RELS[pick_weighted(rng, &weights)];
    let subj_pool = pools.get(sp);
    let subject = subj_pool[rng.below(subj_pool.len())];
    let obj_pool = pools.get(op);
    let premises = rel == R::DependsOnLogically && rng.below(10) < 4;
    let want = if premises { 2 + rng.below(2) } else { 1 };
    let mut targets = Vec::with_capacity(want);
    for _ in 0..want * 4 {
        let t = obj_pool[rng.skewed(obj_pool.len())];
        if t != subject && !targets.contains(&t) {
            targets.push(t);
        }
        if targets.len() == want {
            break;
        }
    }
    let mut context = Vec::new();
    if rel == R::DependsOnLogically && rng.below(10) == 0 {
        let areas = pools.get(Pool::Of(T::Area));
        let proofs = pools.get(Pool::Of(T::Proof));
        context.push(("context", areas[rng.below(areas.len())]));
        let via = proofs[rng.below(proofs.len())];
        if via != subject && !targets.contains(&via) {
            context.push(("via", via));
        }
    }
    Draft { rel, subject, targets, premises, context, group: None }
}

fn insert_claims(tx: &rusqlite::Transaction<'_>, cfg: &SynthConfig, rng: &mut Rng, pools: &Pools) -> Result<SynthReport> {
    let mut ins_claim = tx.prepare("INSERT INTO claims VALUES (?1, ?2, ?3, ?4, NULL, ?5, NULL, ?6, ?7)")?;
    let mut ins_part = tx.prepare("INSERT INTO claim_participants VALUES (?1, ?2, ?3, ?4)")?;
    let mut ins_prov = tx.prepare(
        "INSERT INTO provenance VALUES (?1, NULL, ?2, ?3, 'synth', '1', ?4, ?5, NULL)",
    )?;
    let mut rep = SynthReport {
        entities: 0, claims: 0, participants: 0, current_edges: 0, conjunctions: 0, alternative_groups: 0, secs: 0.0,
    };
    let mut pending: Option<Draft> = None;
    let mut data = String::with_capacity(512);
    let mut i: u64 = 0;
    while rep.current_edges < cfg.edges {
        let mut d = match pending.take() {
            Some(d) => d,
            None => draft(rng, pools),
        };
        if d.targets.is_empty() {
            continue;
        }
        // 5% of dependency claims get an alternative proof: same subject, same relation, one group
        if d.rel == R::DependsOnLogically && d.group.is_none() && rng.below(20) == 0 {
            let g = format!("g:{:016x}", mix(i ^ 0xA17));
            let mut alt = draft(rng, pools);
            while alt.rel != R::DependsOnLogically {
                alt = draft(rng, pools);
            }
            alt.subject = d.subject;
            alt.targets.retain(|&t| t != d.subject);
            alt.context.retain(|&(_, n)| n != d.subject);
            if !alt.targets.is_empty() {
                alt.group = Some(g.clone());
                d.group = Some(g);
                pending = Some(alt);
                rep.alternative_groups += 1;
            }
        }
        let id = claim_id(i, cfg.seed);
        let roll = rng.below(1000);
        let state = match roll {
            0..850 => S::Extracted,
            850..970 => S::ModelProposed,
            970..980 => S::Disputed,
            980..990 => S::Superseded,
            _ => S::Retracted,
        };
        let superseded_by = (state == S::Superseded).then(|| claim_id(i + 1, cfg.seed));
        let (src, lic) = SOURCES[(i % 3) as usize];
        let locator = format!("§{}", i / 50);
        let tgt_role = if d.premises { "premise" } else { "object" };

        data.clear();
        let _ = write!(data, r#"{{"id":"{id}","relation":"{}","participants":["#, d.rel.as_str());
        let subj = entity_id(d.subject, cfg.seed);
        let _ = write!(data, r#"{{"role":"subject","entity":"{subj}"}}"#);
        let mut parts = vec![("subject", subj)];
        for &t in &d.targets {
            let e = entity_id(t, cfg.seed);
            let _ = write!(data, r#",{{"role":"{tgt_role}","entity":"{e}"}}"#);
            parts.push((tgt_role, e));
        }
        for &(role, n) in &d.context {
            let e = entity_id(n, cfg.seed);
            let _ = write!(data, r#",{{"role":"{role}","entity":"{e}"}}"#);
            parts.push((role, e));
        }
        let _ = write!(
            data,
            r#"],"provenance":[{{"source_id":"{src}","locator":"{locator}","extractor":"synth","extractor_version":"1","license":"{lic}","retrieved_at":"{TS}"}}],"evidence_state":"{}""#,
            state.as_str()
        );
        if let Some(g) = &d.group {
            let _ = write!(data, r#","alternative_group":"{g}""#);
        }
        if let Some(s) = &superseded_by {
            let _ = write!(data, r#","superseded_by":"{s}""#);
        }
        if rng.below(33) == 0 {
            let _ = write!(data, r#","assumptions":["{}"]"#, ASSUMPTIONS[rng.below(ASSUMPTIONS.len())]);
        }
        if rng.below(50) == 0 {
            let _ = write!(data, r#","validity_domain":"{}""#, ASSUMPTIONS[rng.below(ASSUMPTIONS.len())]);
        }
        data.push('}');

        ins_claim.execute(params![id, d.rel.as_str(), state.as_str(), d.group, superseded_by, data, TS])?;
        for (pos, (role, e)) in parts.iter().enumerate() {
            ins_part.execute(params![id, role, e, pos as i64])?;
        }
        ins_prov.execute(params![id, src, locator, lic, TS])?;
        rep.claims += 1;
        rep.participants += parts.len() as u64;
        rep.conjunctions += d.premises as u64;
        if state.is_current() {
            rep.current_edges += d.targets.len() as u64;
        }
        i += 1;
    }
    Ok(rep)
}
