//! The composition algebra of shapes/composition.json, compiled to a lookup table.
//!
//! A route is a word over steps `(relation, direction)`.  Each step maps to a
//! *kind* = (base conclusion, modality); the route's conclusion is the product of
//! its kinds.  Bases compose through the sparse `compose` list (everything else is
//! `none`, which absorbs); modality counts analogies (`max_per_path`, R10).  The
//! product is a finite monoid with identity `EMPTY`, so the search can carry one
//! byte per state and splice a prefix with a suffix (bidirectional search needs
//! associativity, which `check` verifies over every triple).

use atlas_core::{EvidenceState, Projection, RelationType};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::Objective;

/// A kind index (base * modalities + modality).  `NONE` = the route authorizes nothing.
pub type Kind = u8;
pub const NONE: Kind = u8::MAX;
/// The empty route (identity).
pub const EMPTY: Kind = 0;
/// Strongest value of the strength scale.
pub const MAX_STRENGTH: u8 = 5;

/// Direction a step walks a stored claim (subject -> target is forward).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Way {
    Forward = 0,
    Converse = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conjunction {
    /// Premises are listed for the reader; the step is complete on its own.
    Informative,
    /// R7: every premise must be satisfied for the step to count.
    RequireAll,
}

/// What one step type authorizes.
#[derive(Clone, Debug)]
pub struct StepSpec {
    pub kind: Kind,
    pub conjunction: Conjunction,
    pub intuition: f64,
    pub assumptions_union: bool,
    pub authorizes_es: String,
    pub authorizes_en: String,
}

impl StepSpec {
    pub fn authorizes(&self, lang: &str) -> &str {
        if lang == "en" { &self.authorizes_en } else { &self.authorizes_es }
    }
}

/// A named conclusion (what a whole route authorizes).
#[derive(Clone, Debug)]
pub struct ConclusionInfo {
    pub name: String,
    pub strength: u8,
    pub mathematical: bool,
    pub hypothetical: bool,
    pub family: String,
    pub es: String,
    pub en: String,
}

impl ConclusionInfo {
    pub fn text(&self, lang: &str) -> &str {
        if lang == "en" { &self.en } else { &self.es }
    }
}

/// Weight names a route cost may use (composition.json `weights.terms`).
pub const WEIGHT_TERMS: [&str; 8] =
    ["length", "evidence", "formal", "new", "missing_premise", "diversity", "intuition", "conclusion"];

// ------------------------------------------------------------------ JSON shape

#[derive(Deserialize)]
struct Text {
    es: String,
    en: String,
}

#[derive(Deserialize)]
struct RawConclusion {
    strength: u8,
    mathematical: bool,
    family: String,
    #[serde(default)]
    hypothetical: bool,
    es: String,
    en: String,
}

#[derive(Deserialize)]
struct RawModality {
    strength_cap: u8,
    max_per_path: u8,
    names: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RawStepOverride {
    kind: String,
    #[serde(default)]
    modality: Option<String>,
}

#[derive(Deserialize)]
struct RawConverse {
    kind: String,
    #[serde(default)]
    conjunction: Option<String>,
    intuition: f64,
    authorizes: Text,
}

#[derive(Deserialize)]
struct RawRelation {
    chain: String,
    single_hop: String,
    chain_result: String,
    assumptions: String,
    intuition: f64,
    authorizes: Text,
    #[serde(default)]
    step: Option<RawStepOverride>,
    #[serde(default)]
    converse: Option<RawConverse>,
    #[serde(default)]
    transparent_in: Vec<String>,
}

#[derive(Deserialize)]
struct RawCompose {
    left: Vec<String>,
    right: Vec<String>,
    result: String,
    #[serde(default)]
    rule: String,
}

#[derive(Deserialize)]
struct RawPair {
    first: String,
    then: String,
    result: String,
}

#[derive(Deserialize)]
struct RawExpect {
    path: Vec<String>,
    conclusion: String,
    #[serde(default)]
    rule: String,
}

#[derive(Deserialize)]
struct RawWeights {
    objectives: BTreeMap<String, BTreeMap<String, f64>>,
}

#[derive(Deserialize)]
struct RawTable {
    version: u32,
    conclusions: BTreeMap<String, RawConclusion>,
    modalities: BTreeMap<String, RawModality>,
    relations: BTreeMap<String, RawRelation>,
    compose: Vec<RawCompose>,
    pairs: Vec<RawPair>,
    expect: Vec<RawExpect>,
    evidence_penalty: BTreeMap<String, serde_json::Value>,
    weights: RawWeights,
}

// ------------------------------------------------------------------ compiled table

/// shapes/composition.json, parsed, checked against the vocabulary and compiled.
pub struct CompositionTable {
    pub version: u32,
    /// Base conclusions; index 0 is the identity `empty`.
    bases: Vec<ConclusionInfo>,
    /// Named conclusions of hypothetical kinds, by base index (index into `hyp_names`).
    hyp_conclusion: Vec<usize>,
    hyp_names: Vec<ConclusionInfo>,
    hyp_cap: u8,
    /// Modality values 0..=max_hyp.
    modalities: u8,
    none: ConclusionInfo,
    /// K x K product table.
    table: Vec<Kind>,
    kinds: usize,
    steps: [[Option<StepSpec>; 2]; RelationType::COUNT],
    evidence_penalty: [f64; EvidenceState::COUNT],
    objectives: HashMap<Objective, BTreeMap<String, f64>>,
    /// Label subsumption, see `dominates`: [min_strength][side][a * kinds + b].
    dom: Vec<bool>,
    /// What `check` verified (for reports and tests).
    pub checked: CheckReport,
}

#[derive(Clone, Debug, Default)]
pub struct CheckReport {
    pub kinds: usize,
    pub triples: usize,
    pub expectations: usize,
    pub pairs: usize,
}

fn fail(msg: impl Into<String>) -> atlas_core::Error {
    atlas_core::Error::Format(format!("composition.json: {}", msg.into()))
}

impl CompositionTable {
    pub fn load(path: &Path) -> atlas_core::Result<CompositionTable> {
        let text = std::fs::read_to_string(path).map_err(|e| atlas_core::Error::io(path, e))?;
        Self::from_json_str(&text)
    }

    pub fn from_json_str(text: &str) -> atlas_core::Result<CompositionTable> {
        let raw: RawTable = serde_json::from_str(text).map_err(|e| fail(e.to_string()))?;
        let mut t = build(&raw)?;
        let (errors, triples) = check(&t, &raw);
        t.checked.triples = triples;
        if !errors.is_empty() {
            return Err(fail(format!("{} consistency error(s): {}", errors.len(), errors.join("; "))));
        }
        t.checked.expectations = raw.expect.len();
        t.checked.pairs = raw.pairs.len();
        t.dom = dominance(&t);
        Ok(t)
    }

    pub fn num_kinds(&self) -> usize {
        self.kinds
    }

    #[inline]
    pub fn compose(&self, a: Kind, b: Kind) -> Kind {
        if a == NONE || b == NONE {
            return NONE;
        }
        self.table[a as usize * self.kinds + b as usize]
    }

    #[inline]
    pub fn step(&self, rel: RelationType, way: Way) -> Option<&StepSpec> {
        self.steps[rel as usize][way as usize].as_ref()
    }

    /// Kind of a one-step route, NONE when the step cannot be walked that way.
    #[inline]
    pub fn step_kind(&self, rel: RelationType, way: Way) -> Kind {
        self.step(rel, way).map_or(NONE, |s| s.kind)
    }

    fn base_of(&self, k: Kind) -> usize {
        k as usize / self.modalities as usize
    }

    fn modality_of(&self, k: Kind) -> u8 {
        k % self.modalities
    }

    pub fn is_hypothetical(&self, k: Kind) -> bool {
        k != NONE && self.modality_of(k) > 0
    }

    /// The conclusion a route of kind `k` authorizes.
    pub fn conclusion(&self, k: Kind) -> &ConclusionInfo {
        if k == NONE {
            return &self.none;
        }
        let b = self.base_of(k);
        if self.modality_of(k) > 0 { &self.hyp_names[self.hyp_conclusion[b]] } else { &self.bases[b] }
    }

    pub fn strength(&self, k: Kind) -> u8 {
        if k == EMPTY {
            return MAX_STRENGTH;
        }
        if k == NONE {
            return 0;
        }
        // a hypothetical kind is never stronger than its base nor than the cap
        let base = self.bases[self.base_of(k)].strength;
        if self.is_hypothetical(k) { base.min(self.hyp_cap) } else { base }
    }

    pub fn evidence_penalty(&self, s: EvidenceState) -> f64 {
        self.evidence_penalty[s as usize]
    }

    /// Default weights of an objective (composition.json `weights.objectives`).
    pub fn objective_weights(&self, o: Objective) -> &BTreeMap<String, f64> {
        &self.objectives[&o]
    }

    /// Does a partial route of kind `a` subsume one of kind `b` (same node, no
    /// higher cost, no more hops)?  True when every completion that makes `b` an
    /// authorized route (strength >= `min_strength`) makes `a` one too, at least as
    /// strong.  `prefix`: completions are appended (forward side); otherwise
    /// prepended (backward side).  The route cost depends on the conclusion only
    /// through its strength, so the subsumed label can never end a better route.
    #[inline]
    pub fn dominates(&self, min_strength: u8, prefix: bool, a: Kind, b: Kind) -> bool {
        let k = self.kinds;
        let i = ((min_strength.min(MAX_STRENGTH) as usize * 2 + prefix as usize) * k + a as usize) * k + b as usize;
        self.dom[i]
    }

    /// Conclusion of a word such as `["analogous_to", "applied_in^-1"]`.
    pub fn word_conclusion(&self, word: &[&str]) -> atlas_core::Result<&ConclusionInfo> {
        let mut k = EMPTY;
        for w in word {
            let (rel, way) = parse_step(w).map_err(fail)?;
            k = self.compose(k, self.step_kind(rel, way));
        }
        Ok(self.conclusion(k))
    }
}

/// Precompute `dominates` for every min_strength and side (K^3 per table, at load).
fn dominance(t: &CompositionTable) -> Vec<bool> {
    let k = t.kinds;
    let mut dom = vec![false; (MAX_STRENGTH as usize + 1) * 2 * k * k];
    for ms in 0..=MAX_STRENGTH {
        let valid = |x: Kind| x != NONE && x != EMPTY && t.strength(x) >= ms;
        for prefix in [false, true] {
            for a in 0..k as Kind {
                for b in 0..k as Kind {
                    let ok = a == b
                        || (a != EMPTY
                            && b != EMPTY
                            && (0..k as Kind).all(|s| {
                                let (x, y) = if prefix { (t.compose(a, s), t.compose(b, s)) } else { (t.compose(s, a), t.compose(s, b)) };
                                !valid(y) || (valid(x) && t.strength(x) >= t.strength(y))
                            }));
                    let i = ((ms as usize * 2 + prefix as usize) * k + a as usize) * k + b as usize;
                    dom[i] = ok;
                }
            }
        }
    }
    dom
}

fn parse_step(s: &str) -> Result<(RelationType, Way), String> {
    let (name, way) = match s.strip_suffix("^-1") {
        Some(n) => (n, Way::Converse),
        None => (s, Way::Forward),
    };
    Ok((name.parse::<RelationType>().map_err(|e| e.to_string())?, way))
}

fn build(raw: &RawTable) -> atlas_core::Result<CompositionTable> {
    // bases: identity first, then every non-hypothetical conclusion except none
    let mut bases = vec![ConclusionInfo {
        name: "empty".into(),
        strength: MAX_STRENGTH,
        mathematical: true,
        hypothetical: false,
        family: "identity".into(),
        es: "ruta vacia".into(),
        en: "empty route".into(),
    }];
    let mut hyp_names = Vec::new();
    let mut none = None;
    for (name, c) in &raw.conclusions {
        let info = ConclusionInfo {
            name: name.clone(),
            strength: c.strength,
            mathematical: c.mathematical,
            hypothetical: c.hypothetical,
            family: c.family.clone(),
            es: c.es.clone(),
            en: c.en.clone(),
        };
        if c.strength > MAX_STRENGTH {
            return Err(fail(format!("conclusion {name}: strength {} > {MAX_STRENGTH}", c.strength)));
        }
        match (name.as_str(), c.hypothetical) {
            ("none", _) => none = Some(info),
            (_, true) => hyp_names.push(info),
            (_, false) => bases.push(info),
        }
    }
    let none = none.ok_or_else(|| fail("conclusion 'none' is missing"))?;
    let base_ix: HashMap<&str, usize> = bases.iter().enumerate().map(|(i, b)| (b.name.as_str(), i)).collect();
    let base = |n: &str| base_ix.get(n).copied().ok_or_else(|| fail(format!("unknown base conclusion {n:?}")));

    let hyp = raw.modalities.get("hypothetical").ok_or_else(|| fail("modalities.hypothetical is missing"))?;
    if raw.modalities.len() != 1 {
        return Err(fail("only the 'hypothetical' modality is supported"));
    }
    let modalities = hyp.max_per_path.checked_add(1).filter(|&m| m <= 4).ok_or_else(|| fail("max_per_path must be 0..=3"))?;
    let hyp_ix = |n: &str| {
        hyp_names.iter().position(|h| h.name == n).ok_or_else(|| fail(format!("hypothetical name {n:?} is not a hypothetical conclusion")))
    };
    let default_hyp = hyp_ix(hyp.names.get("*").ok_or_else(|| fail("modalities.hypothetical.names needs '*'"))?)?;
    let mut hyp_conclusion = vec![default_hyp; bases.len()];
    for (b, n) in &hyp.names {
        if b != "*" {
            hyp_conclusion[base(b)?] = hyp_ix(n)?;
        }
    }

    // base product table (identity row/column, then the sparse list)
    let nb = bases.len();
    let mut bt = vec![NONE; nb * nb];
    for i in 0..nb {
        bt[i] = i as u8; // empty o i
        bt[i * nb] = i as u8; // i o empty
    }
    let mut origin: HashMap<(usize, usize), String> = HashMap::new();
    for (n, e) in raw.compose.iter().enumerate() {
        for l in &e.left {
            for r in &e.right {
                let (li, ri) = (base(l)?, base(r)?);
                let res = match e.result.as_str() {
                    "=left" => li,
                    "=right" => ri,
                    other => base(other)?,
                };
                let cell = &mut bt[li * nb + ri];
                if *cell != NONE && *cell as usize != res {
                    return Err(fail(format!(
                        "compose[{n}] ({}) redefines {l} o {r}: {} vs {} (first from {})",
                        e.rule, bases[*cell as usize].name, bases[res].name, origin[&(li, ri)]
                    )));
                }
                *cell = res as u8;
                origin.entry((li, ri)).or_insert_with(|| format!("compose[{n}] {}", e.rule));
            }
        }
    }

    // expanded kinds: base * m + modality; modalities add and overflow to none
    let m = modalities as usize;
    let kinds = nb * m;
    if kinds >= NONE as usize {
        return Err(fail("too many kinds"));
    }
    let mut table = vec![NONE; kinds * kinds];
    for a in 0..kinds {
        for b in 0..kinds {
            let (ba, ma, bb, mb) = (a / m, a % m, b / m, b % m);
            let base_res = bt[ba * nb + bb];
            if base_res != NONE && ma + mb < m {
                table[a * kinds + b] = (base_res as usize * m + ma + mb) as u8;
            }
        }
    }

    let kind_of = |name: &str, modality: Option<&str>| -> atlas_core::Result<Kind> {
        let md = match modality {
            None | Some("asserted") => 0,
            Some("hypothetical") => 1,
            Some(x) => return Err(fail(format!("unknown modality {x:?}"))),
        };
        Ok((base(name)? * m + md) as Kind)
    };

    let mut steps: [[Option<StepSpec>; 2]; RelationType::COUNT] = Default::default();
    for (name, r) in &raw.relations {
        let rel: RelationType = name.parse().map_err(|e: atlas_core::vocab::UnknownVariant| fail(e.to_string()))?;
        let assumptions_union = match r.assumptions.as_str() {
            "union" => true,
            "ignore" => false,
            x => return Err(fail(format!("{name}.assumptions: {x:?}"))),
        };
        // a relation that concludes nothing on its own is never walked (member_of_concept)
        if r.single_hop == "none" && r.step.is_none() {
            if r.converse.is_some() {
                return Err(fail(format!("{name}: single_hop none is not walkable, so it cannot declare a converse")));
            }
            continue;
        }
        let fwd_kind = match &r.step {
            Some(o) => kind_of(&o.kind, o.modality.as_deref())?,
            None => kind_of(&r.single_hop, None)?,
        };
        steps[rel as usize][0] = Some(StepSpec {
            kind: fwd_kind,
            conjunction: Conjunction::Informative,
            intuition: r.intuition,
            assumptions_union,
            authorizes_es: r.authorizes.es.clone(),
            authorizes_en: r.authorizes.en.clone(),
        });
        if let Some(c) = &r.converse {
            let conjunction = match c.conjunction.as_deref() {
                None | Some("informative") => Conjunction::Informative,
                Some("require_all") => Conjunction::RequireAll,
                Some(x) => return Err(fail(format!("{name}.converse.conjunction: {x:?}"))),
            };
            steps[rel as usize][1] = Some(StepSpec {
                kind: kind_of(&c.kind, None)?,
                conjunction,
                intuition: c.intuition,
                assumptions_union,
                authorizes_es: c.authorizes.es.clone(),
                authorizes_en: c.authorizes.en.clone(),
            });
        }
    }
    // implicit converses from the vocabulary: symmetric relations walk both ways as
    // themselves; declared inverses walk backwards as the inverse's forward step.
    for rel in RelationType::ALL {
        let spec = rel.spec();
        let implicit = if spec.symmetric {
            steps[*rel as usize][0].clone()
        } else if let Some(inv) = spec.inverse {
            steps[inv as usize][0].clone()
        } else {
            None
        };
        if let Some(s) = implicit {
            if steps[*rel as usize][1].is_some() {
                return Err(fail(format!("{rel}: converse is implied by the vocabulary (symmetric/inverse); do not declare it")));
            }
            steps[*rel as usize][1] = Some(s);
        }
    }

    let mut evidence_penalty = [f64::INFINITY; EvidenceState::COUNT];
    for (k, v) in &raw.evidence_penalty {
        if k == "note" {
            continue;
        }
        let s: EvidenceState = k.parse().map_err(|e: atlas_core::vocab::UnknownVariant| fail(e.to_string()))?;
        evidence_penalty[s as usize] = v.as_f64().ok_or_else(|| fail(format!("evidence_penalty.{k} is not a number")))?;
    }

    let mut objectives = HashMap::new();
    for (k, w) in &raw.weights.objectives {
        let o: Objective =
            serde_json::from_value(serde_json::Value::String(k.clone())).map_err(|e| fail(format!("objective {k}: {e}")))?;
        objectives.insert(o, w.clone());
    }

    Ok(CompositionTable {
        version: raw.version,
        bases,
        hyp_conclusion,
        hyp_names,
        hyp_cap: hyp.strength_cap,
        modalities,
        none,
        table,
        kinds,
        steps,
        evidence_penalty,
        objectives,
        dom: Vec::new(),
        checked: CheckReport { kinds, ..Default::default() },
    })
}

// ------------------------------------------------------------------ consistency checker

const FAMILIES: [&str; 11] = [
    "identity", "proof", "structure", "formal", "application", "learning", "biblio", "history", "documentary",
    "hypothetical", "none",
];

/// Every check; an empty list means the table is consistent.
fn check(t: &CompositionTable, raw: &RawTable) -> (Vec<String>, usize) {
    let mut err = Vec::new();
    check_vocabulary(t, raw, &mut err);
    let triples = check_monoid(t, &mut err);
    check_relations(t, raw, &mut err);
    check_examples(t, raw, &mut err);
    check_costs(t, &mut err);
    (err, triples)
}

fn check_vocabulary(t: &CompositionTable, raw: &RawTable, err: &mut Vec<String>) {
    for c in t.bases.iter().chain(&t.hyp_names).chain([&t.none]) {
        if !FAMILIES.contains(&c.family.as_str()) {
            err.push(format!("conclusion {}: unknown family {:?}", c.name, c.family));
        }
    }
    for rel in RelationType::ALL {
        if !raw.relations.contains_key(rel.as_str()) {
            err.push(format!("relation {rel} has no entry"));
        }
    }
    for (name, r) in &raw.relations {
        for k in [&r.single_hop, &r.chain_result] {
            if !raw.conclusions.contains_key(k) {
                err.push(format!("{name}: unknown conclusion {k:?}"));
            }
        }
        for f in &r.transparent_in {
            if !FAMILIES.contains(&f.as_str()) {
                err.push(format!("{name}.transparent_in: unknown family {f:?}"));
            }
        }
    }
}

/// Identity, associativity, monotone non-strengthening and taint closure.
fn check_monoid(t: &CompositionTable, err: &mut Vec<String>) -> usize {
    let k = t.kinds as Kind;
    let name = |x: Kind| format!("{}{}", t.conclusion(x).name, if t.is_hypothetical(x) { "?" } else { "" });
    let mut assoc_failures = 0usize;
    let mut triples = 0usize;
    for a in 0..k {
        if t.compose(EMPTY, a) != a || t.compose(a, EMPTY) != a {
            err.push(format!("empty is not an identity for {}", name(a)));
        }
        for b in 0..k {
            let ab = t.compose(a, b);
            if a != EMPTY && b != EMPTY && ab != NONE {
                if t.strength(ab) > t.strength(a).min(t.strength(b)) {
                    err.push(format!("monotone: {} o {} = {} is stronger than an operand", name(a), name(b), name(ab)));
                }
                let learning = |x: Kind| t.conclusion(x).family == "learning" && !t.is_hypothetical(x);
                if (learning(a) || learning(b)) && !(learning(ab) || t.is_hypothetical(ab)) {
                    err.push(format!("R5 cap: {} o {} = {} escapes study_route", name(a), name(b), name(ab)));
                }
                if (t.is_hypothetical(a) || t.is_hypothetical(b)) && !t.is_hypothetical(ab) {
                    err.push(format!("R3 taint: {} o {} = {} drops the hypothesis", name(a), name(b), name(ab)));
                }
                if (t.strength(a) == 0 || t.strength(b) == 0) && t.conclusion(ab).mathematical {
                    err.push(format!("R4: {} o {} = {} is mathematical", name(a), name(b), name(ab)));
                }
            }
            for c in 0..k {
                triples += 1;
                let l = t.compose(ab, c);
                let r = t.compose(a, t.compose(b, c));
                if l != r {
                    assoc_failures += 1;
                    if assoc_failures <= 5 {
                        err.push(format!(
                            "not associative: ({} o {}) o {} = {} but {} o ({} o {}) = {}",
                            name(a), name(b), name(c), name(l), name(a), name(b), name(c), name(r)
                        ));
                    }
                }
            }
        }
    }
    if assoc_failures > 5 {
        err.push(format!("... {assoc_failures} associativity failures in total"));
    }
    triples
}

fn check_relations(t: &CompositionTable, raw: &RawTable, err: &mut Vec<String>) {
    for rel in RelationType::ALL {
        let spec = rel.spec();
        let Some(r) = raw.relations.get(rel.as_str()) else { continue };
        let f = t.step_kind(*rel, Way::Forward);
        let c = t.step_kind(*rel, Way::Converse);
        if (r.chain == "transitive") != spec.transitive {
            err.push(format!("{rel}: chain {:?} disagrees with RELATIONS.transitive = {}", r.chain, spec.transitive));
        }
        if t.conclusion(f).name != r.single_hop {
            err.push(format!("{rel}: single_hop says {} but the step concludes {}", r.single_hop, t.conclusion(f).name));
        }
        if t.conclusion(t.compose(f, f)).name != r.chain_result {
            err.push(format!("{rel}: chain_result says {} but step o step = {}", r.chain_result, t.conclusion(t.compose(f, f)).name));
        }
        if spec.transitive {
            for (way, k) in [("forward", f), ("converse", c)] {
                if k != NONE && t.compose(k, k) != k {
                    err.push(format!("R1: {rel} is transitive but its {way} step does not chain to itself"));
                }
            }
        } else if t.strength(f) > 0 && t.compose(f, f) == f {
            err.push(format!("R1: {rel} is not transitive but step o step = step"));
        }
        if matches!(spec.projection, Projection::History | Projection::Biblio) {
            for k in [f, c] {
                if k != NONE && (t.strength(k) > 0 || t.conclusion(k).mathematical) {
                    err.push(format!("R4: {rel} must conclude nothing semantic"));
                }
            }
        }
        if spec.allows_premises {
            let ok = t.step(*rel, Way::Converse).is_some_and(|s| s.conjunction == Conjunction::RequireAll);
            if !ok {
                err.push(format!("R7: {rel} allows premises, so its converse must declare conjunction require_all"));
            }
        }
        if !r.transparent_in.is_empty() {
            for (i, b) in t.bases.iter().enumerate().skip(1) {
                if r.transparent_in.contains(&b.family) {
                    let bk = (i * t.modalities as usize) as Kind;
                    if t.compose(f, bk) != bk || t.compose(bk, f) != bk {
                        err.push(format!("R2: {rel} is declared transparent for {} but does not compose as identity", b.name));
                    }
                }
            }
        }
    }
}

fn check_examples(t: &CompositionTable, raw: &RawTable, err: &mut Vec<String>) {
    for p in &raw.pairs {
        match t.word_conclusion(&[&p.first, &p.then]) {
            Ok(c) if c.name == p.result => {}
            Ok(c) => err.push(format!("pair {} o {}: expected {}, table gives {}", p.first, p.then, p.result, c.name)),
            Err(e) => err.push(e.to_string()),
        }
    }
    for e in &raw.expect {
        let word: Vec<&str> = e.path.iter().map(String::as_str).collect();
        match t.word_conclusion(&word) {
            Ok(c) if c.name == e.conclusion => {}
            Ok(c) => err.push(format!("expect {:?} ({}): expected {}, table gives {}", e.path, e.rule, e.conclusion, c.name)),
            Err(x) => err.push(x.to_string()),
        }
    }
}

fn check_costs(t: &CompositionTable, err: &mut Vec<String>) {
    use EvidenceState as S;
    let order = [S::FormallyLinked, S::EditoriallyReviewed, S::Extracted, S::ModelProposed, S::Disputed];
    for w in order.windows(2) {
        let (a, b) = (t.evidence_penalty(w[0]), t.evidence_penalty(w[1]));
        if !(a.is_finite() && b.is_finite() && a >= 0.0 && a <= b) {
            err.push(format!("evidence_penalty must be finite, >= 0 and non-decreasing: {} = {a}, {} = {b}", w[0], w[1]));
        }
    }
    for o in Objective::ALL {
        match t.objectives.get(&o) {
            None => err.push(format!("weights.objectives: {o:?} is missing")),
            Some(w) => {
                for (k, v) in w {
                    if !WEIGHT_TERMS.contains(&k.as_str()) || !v.is_finite() || *v < 0.0 {
                        err.push(format!("weights.objectives.{o:?}: bad weight {k} = {v}"));
                    }
                }
            }
        }
    }
    for rel in RelationType::ALL {
        for way in [Way::Forward, Way::Converse] {
            if let Some(s) = t.step(*rel, way) {
                if !(s.intuition.is_finite() && s.intuition >= 0.0) {
                    err.push(format!("{rel}: intuition must be >= 0"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> CompositionTable {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shapes/composition.json");
        CompositionTable::load(&p).expect("shipped table is consistent")
    }

    #[test]
    fn shipped_table_is_a_monoid() {
        let t = shipped();
        assert!(t.num_kinds() > 10);
        assert_eq!(t.conclusion(EMPTY).name, "empty");
    }

    #[test]
    fn subsumption_is_reflexive_and_never_trades_a_hypothesis_for_a_claim() {
        let t = shipped();
        let k = t.num_kinds() as Kind;
        let mut pairs = 0;
        for ms in 0..=MAX_STRENGTH {
            for prefix in [false, true] {
                for a in 0..k {
                    assert!(t.dominates(ms, prefix, a, a));
                    for b in 0..k {
                        if a != b && t.dominates(ms, prefix, a, b) {
                            pairs += 1;
                            // a hypothesis never subsumes an asserted route that can still conclude something
                            if t.is_hypothetical(a) && !t.is_hypothetical(b) && b != EMPTY {
                                assert!(t.strength(b) <= t.strength(a) || ms > t.strength(a), "{a} {b}");
                            }
                        }
                    }
                }
            }
        }
        eprintln!("subsumption pairs (a != b, all min_strength and sides): {pairs}");
        // logical dependency is not subsumed by a study route (R5)
        let dep = t.step_kind(RelationType::DependsOnLogically, Way::Forward);
        let pre = t.step_kind(RelationType::PrerequisiteFor, Way::Forward);
        assert!(!t.dominates(1, true, pre, dep));
    }

    fn word(t: &CompositionTable, w: &[&str]) -> String {
        t.word_conclusion(w).map(|c| c.name.clone()).unwrap_or_else(|e| format!("error: {e}"))
    }

    #[test]
    fn adr0005_said_equivalence_and_instance_compose_only_as_declared() {
        let t = shipped();
        // 7b: said_equivalent_to is one hop and composes with nothing (A~B, B~C does not give A~C)
        assert_eq!(word(&t, &["said_equivalent_to"]), "said_equivalence");
        assert!(!t.conclusion(t.step_kind(RelationType::SaidEquivalentTo, Way::Forward)).mathematical);
        for w in [["said_equivalent_to", "said_equivalent_to"], ["said_equivalent_to", "equivalent_to"],
                  ["equivalent_to", "said_equivalent_to"], ["said_equivalent_to^-1", "said_equivalent_to"]] {
            assert_eq!(word(&t, &w), "none", "{w:?}");
        }
        // 7c: instance_of o special_case_of* -> instance_of; instance_of o instance_of does not compose
        assert_eq!(word(&t, &["instance_of"]), "instance");
        assert_eq!(word(&t, &["instance_of", "special_case_of"]), "instance");
        assert_eq!(word(&t, &["instance_of", "special_case_of", "special_case_of", "special_case_of"]), "instance");
        assert_eq!(word(&t, &["instance_of", "instance_of"]), "none");
        assert_eq!(word(&t, &["instance_of", "special_case_of", "instance_of"]), "none");
        assert_eq!(word(&t, &["special_case_of", "instance_of"]), "none");
    }

    #[test]
    fn adr0005_no_route_through_citation_or_authorship_is_mathematical_or_pedagogical() {
        use RelationType as R;
        let t = shipped();
        let tainted = [R::Cites, R::AppearsIn, R::IntroducedIn, R::AuthoredBy, R::CopiedBy, R::EditedBy, R::TranslatedBy,
                       R::NamedAfter, R::Influenced];
        let steps: Vec<(RelationType, Kind)> = RelationType::ALL
            .iter()
            .flat_map(|&r| [Way::Forward, Way::Converse].map(|w| (r, t.step_kind(r, w))))
            .filter(|&(_, k)| k != NONE)
            .collect();
        let mut words = 0;
        for &(r1, k1) in &steps {
            for &(r2, k2) in &steps {
                for &(r3, k3) in &steps {
                    let two = tainted.contains(&r1) || tainted.contains(&r2);
                    let three = two || tainted.contains(&r3);
                    for (k, is_tainted) in [(t.compose(k1, k2), two), (t.compose(t.compose(k1, k2), k3), three)] {
                        if k == NONE || !is_tainted {
                            continue;
                        }
                        let c = t.conclusion(k);
                        let cat = crate::explain::category(&c.family, c.mathematical);
                        assert!(!c.mathematical && cat != "mathematical" && cat != "pedagogical",
                                "{r1} {r2} {r3}: {} ({cat})", c.name);
                        words += 1;
                    }
                }
            }
        }
        assert!(words > 0);
    }

    #[test]
    fn checker_rejects_a_non_associative_table() {
        // drop R2b: equivalence no longer transparent in learning routes
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shapes/composition.json");
        let text = std::fs::read_to_string(p).unwrap();
        let broken = text.replace(
            r#""area_membership", "study_route", "application""#,
            r#""area_membership", "application""#,
        );
        assert_ne!(text, broken, "fixture edit applies");
        let e = CompositionTable::from_json_str(&broken).err().expect("must fail").to_string();
        assert!(e.contains("not associative"), "{e}");
    }

    #[test]
    fn checker_rejects_strengthening_and_lost_taint() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shapes/composition.json");
        let text = std::fs::read_to_string(p).unwrap();
        // prerequisite chains claiming logical dependency
        let broken = text.replace(
            r#"{"rule": "R5", "left": ["logical_support"], "right": ["study_route"], "result": "study_route""#,
            r#"{"rule": "R5", "left": ["logical_support"], "right": ["study_route"], "result": "logical_support""#,
        );
        let e = CompositionTable::from_json_str(&broken).err().expect("must fail").to_string();
        assert!(e.contains("monotone") && e.contains("R5 cap"), "{e}");
    }

    #[test]
    fn checker_rejects_citations_with_meaning() {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shapes/composition.json");
        let text = std::fs::read_to_string(p).unwrap();
        let broken = text.replace(
            r#""cites": {"chain": "none", "single_hop": "bibliographic""#,
            r#""cites": {"chain": "none", "single_hop": "logical_dependency""#,
        );
        let e = CompositionTable::from_json_str(&broken).err().expect("must fail").to_string();
        assert!(e.contains("R4"), "{e}");
    }
}
