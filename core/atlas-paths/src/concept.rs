//! Concept-first routes (plan 2026-10-17 «capa conceptual»): the search runs first on
//! the concept graph, whose edges are the snapshot's aggregated concept edges
//! (`Snapshot::concept_edges`: A -> B when a current claim goes from a primary member
//! of A to a primary member of B), then can drill down to the declaration-level claim
//! that witnesses each concept step.
//!
//! Honesty rules:
//! - A concept step walks one relation actually present behind the aggregated edge,
//!   with one of its evidence states actually present; nothing is upgraded.  The
//!   composition algebra is the same as for declarations (mixing still blocks, so a
//!   hub such as «set» never joins A -> set <- B).
//! - The step's evidence is the weakest of the witness claim and the two
//!   `member_of_concept` claims that put its declarations in the concepts: a
//!   heuristic membership makes the step `model_proposed`.
//! - The conclusion is read at concept level: the witnesses of consecutive steps may
//!   be different declarations, so a concept route is never a declaration chain.

use ahash::AHashMap;
use atlas_core::{ClaimIx, Dir, EvidenceState, NodeIx, RelationType, Snapshot};
use rayon::prelude::*;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::time::Instant;

use crate::algebra::{EMPTY, Kind, NONE, Way};
use crate::explain::{category, evidence_rank, explain_path, tr};
use crate::search::{QueryCtx, RawPath, RawStep, Weights};
use crate::{Conclusion, Direction, FoundPath, PathEngine, PathOptions, PathQuery, SourceRef};

/// Popped labels before the concept search gives up (the graph has ~6k nodes; the
/// state space is concepts x kinds x hops, so this bound is never hit in practice).
const MAX_POPS: usize = 500_000;
/// Breaks ties between equally long routes in favour of aggregated edges backed by
/// more declaration-level claims.  ponytail: a fixed tie-break, not a weight; make it
/// an objective term (composition.json `weights`) if readers should tune it.
const SUPPORT_TIEBREAK: f64 = 1e-3;

/// One walkable concept step: (aggregated edge, relation, state), in one direction.
#[derive(Clone, Copy, Debug)]
struct CStep {
    /// Position of the concept reached, in `Snapshot::concepts()`.
    to: u32,
    rel: RelationType,
    /// State of the witness claim.
    dep_state: EvidenceState,
    /// Weakest of the witness claim and the two memberships.
    state: EvidenceState,
    count: u32,
    claim: ClaimIx,
    from_decl: NodeIx,
    to_decl: NodeIx,
}

/// Aggregated concept edges as two CSRs over concept positions: forward (stored
/// direction) and converse (walked against it).
pub struct ConceptGraph {
    off: [Vec<u32>; 2],
    steps: [Vec<CStep>; 2],
}

/// The current `member_of_concept` claim putting `decl` in `concept`.
fn membership(snap: &Snapshot, decl: NodeIx, concept: NodeIx) -> Option<(ClaimIx, EvidenceState)> {
    snap.csr(Dir::Out)
        .edges_rel(decl, RelationType::MemberOfConcept)
        .iter()
        .find(|e| e.nbr == concept && atlas_core::CURRENT_STATES_MASK & e.state.bit() != 0)
        .map(|e| (e.claim, e.state))
}

fn weakest(states: &[EvidenceState]) -> EvidenceState {
    *states.iter().min_by_key(|s| evidence_rank(**s)).expect("non-empty")
}

impl ConceptGraph {
    pub fn build(snap: &Snapshot) -> ConceptGraph {
        let cs = snap.concepts();
        let fwd: Vec<Vec<CStep>> = cs.par_iter().map(|&a| forward_steps(snap, a)).collect();
        let mut conv: Vec<Vec<CStep>> = vec![Vec::new(); cs.len()];
        for (i, list) in fwd.iter().enumerate() {
            for s in list {
                conv[s.to as usize].push(CStep { to: i as u32, from_decl: s.to_decl, to_decl: s.from_decl, ..*s });
            }
        }
        let flat = |lists: Vec<Vec<CStep>>| {
            let mut off = Vec::with_capacity(lists.len() + 1);
            off.push(0u32);
            let mut all = Vec::new();
            for l in lists {
                all.extend(l);
                off.push(all.len() as u32);
            }
            (off, all)
        };
        let ((f_off, f), (c_off, c)) = (flat(fwd), flat(conv));
        ConceptGraph { off: [f_off, c_off], steps: [f, c] }
    }

    fn steps(&self, way: Way, pos: u32) -> &[CStep] {
        let w = way as usize;
        &self.steps[w][self.off[w][pos as usize] as usize..self.off[w][pos as usize + 1] as usize]
    }

    /// Walkable steps (both directions count once each).
    pub fn num_steps(&self) -> usize {
        self.steps[0].len()
    }

    pub fn memory_bytes(&self) -> usize {
        (self.steps[0].len() + self.steps[1].len()) * std::mem::size_of::<CStep>() + (self.off[0].len() + self.off[1].len()) * 4
    }
}

/// Steps out of concept `a` in the stored direction, one per (target, relation, state).
fn forward_steps(snap: &Snapshot, a: NodeIx) -> Vec<CStep> {
    let mut out = Vec::new();
    for e in snap.concept_edges(a) {
        let Some(to) = snap.concept_index(e.target) else { continue };
        for st in e.stats() {
            if st.relation == RelationType::MemberOfConcept {
                continue;
            }
            let from_decl = snap.claim_subject(st.example);
            let to_decl = snap.claim_targets(st.example).iter().copied().find(|&x| snap.primary_concept(x) == Some(e.target));
            let (Some(to_decl), Some((_, ma)), Some(mb)) =
                (to_decl, membership(snap, from_decl, a), to_decl.and_then(|b| membership(snap, b, e.target)))
            else {
                continue; // a witness without both memberships cannot justify the step
            };
            out.push(CStep {
                to: to as u32,
                rel: st.relation,
                dep_state: st.state,
                state: weakest(&[st.state, ma, mb.1]),
                count: st.count,
                claim: st.example,
                from_decl,
                to_decl,
            });
        }
    }
    out
}

// ------------------------------------------------------------------ response

#[derive(Clone, Debug, Serialize)]
pub struct ConceptLevel {
    pub from: ConceptEnd,
    pub to: ConceptEnd,
    pub abstained: bool,
    /// not_a_concept, same_concept, no_route, budget.
    pub abstention: Option<String>,
    pub reason: Option<String>,
    pub route: Option<ConceptRoute>,
    pub expanded: usize,
    pub truncated: bool,
    pub micros: u64,
}

/// How a requested endpoint maps to the concept layer.
#[derive(Clone, Debug, Serialize)]
pub struct ConceptEnd {
    pub requested: String,
    pub concept: Option<String>,
    pub label: Option<String>,
    /// `concept` (the entity is a concept), `primary_concept` (a declaration's
    /// primary concept) or `none`.
    pub via: &'static str,
    /// A concept derived from the Mathlib module hierarchy, not a Wikidata item.
    pub derived: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConceptRoute {
    pub score: f64,
    pub conclusion: Conclusion,
    pub steps: Vec<ConceptStep>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConceptStep {
    pub from: String,
    pub to: String,
    pub from_label: String,
    pub to_label: String,
    pub from_derived: bool,
    pub to_derived: bool,
    pub relation: RelationType,
    pub direction: Direction,
    /// Weakest of the witness claim and the two memberships.
    pub evidence_state: EvidenceState,
    pub authorizes: String,
    pub route_so_far: String,
    /// Declaration-level claims behind this step (this relation and state).
    pub declaration_claims: u32,
    /// Every (relation, state) count behind the aggregated edge, stored direction.
    pub counts: Vec<ConceptCount>,
    pub witness: Witness,
    /// Drill-down (`PathOptions.drill_down`): the declaration-level route of the witness.
    pub drill_down: Option<FoundPath>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ConceptCount {
    pub relation: RelationType,
    pub evidence_state: EvidenceState,
    pub count: u32,
}

/// The declaration-level claim that witnesses a concept step, with its sources and
/// the membership claims that place its declarations in the two concepts.
#[derive(Clone, Debug, Serialize)]
pub struct Witness {
    pub claim_id: String,
    pub from: String,
    pub to: String,
    pub from_label: String,
    pub to_label: String,
    pub evidence_state: EvidenceState,
    /// Premises of the witness claim besides the one walked (conjunctions).
    pub other_premises: usize,
    pub sources: Vec<SourceRef>,
    pub memberships: Vec<Membership>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Membership {
    pub declaration: String,
    pub concept: String,
    pub claim_id: String,
    pub evidence_state: EvidenceState,
    pub sources: Vec<SourceRef>,
}

// ------------------------------------------------------------------ search

fn end(snap: &Snapshot, requested: &str, n: NodeIx, lang: &str) -> (ConceptEnd, Option<u32>) {
    let (c, via) = match snap.concept_index(n) {
        Some(_) => (Some(n), "concept"),
        None => match snap.primary_concept(n) {
            Some(c) => (Some(c), "primary_concept"),
            None => (None, "none"),
        },
    };
    let e = ConceptEnd {
        requested: requested.to_owned(),
        concept: c.map(|c| snap.node_id(c).to_owned()),
        label: c.map(|c| snap.label(c, lang).to_owned()),
        via,
        derived: c.is_some_and(|c| snap.is_derived_concept(c)),
    };
    (e, c.and_then(|c| snap.concept_index(c)).map(|i| i as u32))
}

#[derive(Clone, Copy)]
struct Label {
    pos: u32,
    kind: Kind,
    hops: u8,
    parent: u32,
    step: CStep,
    way: Way,
    g: f64,
}

#[derive(Clone, Copy, PartialEq)]
struct Item {
    key: f64,
    idx: u32,
    done: bool,
}

impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        o.key.total_cmp(&self.key).then(o.idx.cmp(&self.idx))
    }
}

/// Concept-level answer for a query whose endpoints resolved to `s` and `t`.
pub(crate) fn route(
    engine: &PathEngine,
    q: &PathQuery,
    opts: &PathOptions,
    (s, t): (NodeIx, NodeIx),
    w: Weights,
    state_mask: u32,
) -> ConceptLevel {
    let t0 = Instant::now();
    let (snap, lang) = (engine.snapshot, q.lang.as_str());
    let ((from, fs), (to, ft)) = (end(snap, &q.from, s, lang), end(snap, &q.to, t, lang));
    let mut out = ConceptLevel {
        from,
        to,
        abstained: true,
        abstention: None,
        reason: None,
        route: None,
        expanded: 0,
        truncated: false,
        micros: 0,
    };
    let no = |code: &str, es: String, en: String, out: &mut ConceptLevel| {
        out.abstention = Some(code.into());
        out.reason = Some(tr(lang, es, en));
    };
    match (fs, ft) {
        (Some(a), Some(b)) if a == b => no("same_concept",
            format!("Los dos extremos pertenecen al mismo concepto ({}).", snap.node_id(snap.concepts()[a as usize])),
            format!("Both endpoints belong to the same concept ({}).", snap.node_id(snap.concepts()[a as usize])), &mut out),
        (Some(a), Some(b)) => {
            let ctx = {
                let cs = snap.concepts();
                let mut c = QueryCtx::new(snap, &engine.table, &engine.index, cs[a as usize], cs[b as usize], Vec::new(), w, state_mask);
                c.min_strength = opts.min_strength;
                c
            };
            let (found, expanded, truncated) = search(engine, &ctx, a, b, q.max_hops as u8);
            out.expanded = expanded;
            out.truncated = truncated;
            match found {
                Some((labels, last)) => {
                    out.abstained = false;
                    out.route = Some(explain(engine, &ctx, &labels, last, opts.drill_down, lang));
                }
                None if truncated => no("budget",
                    format!("La busqueda entre conceptos agoto su presupuesto ({MAX_POPS} estados)."),
                    format!("The concept search ran out of budget ({MAX_POPS} states)."), &mut out),
                None => no("no_route",
                    format!("Ninguna ruta autorizada de a lo sumo {} pasos une estos conceptos por aristas agregadas.", q.max_hops),
                    format!("No authorized route of at most {} steps joins these concepts through aggregated edges.", q.max_hops), &mut out),
            }
        }
        _ => {
            let who = if fs.is_none() { &q.from } else { &q.to };
            no("not_a_concept",
                format!("{who} no es un concepto ni tiene concepto primario asignado: no hay ruta a nivel de concepto."),
                format!("{who} is not a concept and has no primary concept: no concept-level route."), &mut out);
        }
    }
    out.micros = t0.elapsed().as_micros() as u64;
    out
}

/// Undirected hop distance of every concept to `b` over the concept graph (u8::MAX:
/// unreachable).  A lower bound on the steps of any concept route to `b`, so it prunes
/// by the hop limit and drives A* (`MIN_STEP` per missing hop).
fn hops_to(g: &ConceptGraph, n: usize, b: u32) -> Vec<u8> {
    let mut d = vec![u8::MAX; n];
    d[b as usize] = 0;
    let mut frontier = vec![b];
    let mut next = Vec::new();
    let mut h = 0u8;
    while !frontier.is_empty() && h < u8::MAX - 1 {
        h += 1;
        for &v in &frontier {
            for way in [Way::Forward, Way::Converse] {
                for st in g.steps(way, v) {
                    if d[st.to as usize] == u8::MAX {
                        d[st.to as usize] = h;
                        next.push(st.to);
                    }
                }
            }
        }
        std::mem::swap(&mut frontier, &mut next);
        next.clear();
    }
    d
}

/// Label-setting A* over (concept, kind, hops) with hop-distance potentials; returns
/// the labels and the last one.  Exact within the hop limit: each (concept, kind)
/// keeps the Pareto set of (cost, hops) it settles; a label dominated by the best one
/// already queued for its state is never queued.
fn search(engine: &PathEngine, ctx: &QueryCtx, a: u32, b: u32, max_hops: u8) -> (Option<(Vec<Label>, u32)>, usize, bool) {
    let (g, table) = (&engine.concepts, &engine.table);
    let cs = engine.snapshot.concepts();
    let kinds = table.num_kinds();
    let dist = hops_to(g, cs.len(), b);
    if dist[a as usize] > max_hops {
        return (None, 0, false);
    }
    let min_step = ctx.min_step();
    let h = |pos: u32| min_step * dist[pos as usize] as f64;
    // sparse: a query touches a few thousand of the concepts x kinds states
    let mut settled: AHashMap<usize, u8> = AHashMap::new();
    let mut queued: AHashMap<usize, (f64, u8)> = AHashMap::new();
    let root = CStep { to: a, rel: RelationType::DependsOnLogically, dep_state: EvidenceState::Extracted,
                       state: EvidenceState::Extracted, count: 0, claim: 0, from_decl: 0, to_decl: 0 };
    let mut labels = vec![Label { pos: a, kind: EMPTY, hops: 0, parent: u32::MAX, step: root, way: Way::Forward, g: 0.0 }];
    let mut heap = BinaryHeap::from([Item { key: h(a), idx: 0, done: false }]);
    let mut pops = 0;
    while let Some(it) = heap.pop() {
        if it.done {
            return (Some((labels, it.idx)), pops, false);
        }
        pops += 1;
        if pops > MAX_POPS {
            return (None, pops, true);
        }
        let l = labels[it.idx as usize];
        let slot = l.pos as usize * kinds + l.kind as usize;
        if settled.get(&slot).is_some_and(|&h| h <= l.hops) {
            continue;
        }
        settled.insert(slot, l.hops);
        if l.pos == b {
            continue;
        }
        for way in [Way::Forward, Way::Converse] {
            for st in g.steps(way, l.pos) {
                let hops = l.hops + 1;
                if hops + dist[st.to as usize].min(u8::MAX - 1) > max_hops
                    || ctx.state_mask & st.state.bit() == 0
                    || ctx.state_mask & st.dep_state.bit() == 0
                    || st.to == a
                {
                    continue;
                }
                let kind = table.compose(l.kind, table.step_kind(st.rel, way));
                if kind == NONE || table.strength(kind) < ctx.min_strength {
                    continue;
                }
                let to = st.to as usize * kinds + kind as usize;
                if settled.get(&to).is_some_and(|&h| h <= hops) {
                    continue;
                }
                let raw = RawStep { from: cs[l.pos as usize], to: cs[st.to as usize], claim: st.claim, rel: st.rel, way,
                                    state: st.state, missing: 0 };
                let cost = l.g + ctx.step_cost(&raw) + SUPPORT_TIEBREAK / (1.0 + st.count as f64);
                let q = queued.entry(to).or_insert((f64::INFINITY, u8::MAX));
                if q.0 <= cost && q.1 <= hops {
                    continue;
                }
                if cost <= q.0 && hops <= q.1 {
                    *q = (cost, hops);
                }
                let idx = labels.len() as u32;
                labels.push(Label { pos: st.to, kind, hops, parent: it.idx, step: *st, way, g: cost });
                heap.push(Item { key: cost + h(st.to), idx, done: false });
                if st.to == b {
                    heap.push(Item { key: cost + ctx.terminal_cost(kind), idx, done: true });
                }
            }
        }
    }
    (None, pops, false)
}

fn sources(snap: &Snapshot, c: ClaimIx) -> Vec<SourceRef> {
    snap.claim_provenance(c)
        .map(|p| SourceRef { source_id: p.source_id.into(), locator: p.locator.into(), license: p.license.into() })
        .collect()
}

fn explain(engine: &PathEngine, ctx: &QueryCtx, labels: &[Label], last: u32, drill_down: bool, lang: &str) -> ConceptRoute {
    let (snap, table) = (engine.snapshot, &engine.table);
    let cs = snap.concepts();
    let mut chain = Vec::new();
    let mut i = last;
    while labels[i as usize].parent != u32::MAX {
        chain.push(labels[i as usize]);
        i = labels[i as usize].parent;
    }
    chain.reverse();
    let mut kind = EMPTY;
    let mut weakest_state = EvidenceState::FormallyLinked;
    let (mut heuristic, mut derived, mut conj) = (false, false, false);
    let mut steps = Vec::with_capacity(chain.len());
    for l in &chain {
        let st = l.step;
        let (a, b) = (cs[labels[l.parent as usize].pos as usize], cs[st.to as usize]);
        kind = table.compose(kind, table.step_kind(st.rel, l.way));
        if evidence_rank(st.state) < evidence_rank(weakest_state) {
            weakest_state = st.state;
        }
        // the stored aggregated edge runs subject concept -> target concept
        let (src, dst) = if l.way == Way::Forward { (a, b) } else { (b, a) };
        let counts = snap
            .concept_edges(src)
            .find(|e| e.target == dst)
            .map(|e| e.stats().map(|x| ConceptCount { relation: x.relation, evidence_state: x.state, count: x.count }).collect())
            .unwrap_or_default();
        let memberships: Vec<Membership> = [(st.from_decl, a), (st.to_decl, b)]
            .iter()
            .filter_map(|&(d, c)| {
                membership(snap, d, c).map(|(m, s)| Membership {
                    declaration: snap.node_id(d).into(),
                    concept: snap.node_id(c).into(),
                    claim_id: snap.claim_id(m).into(),
                    evidence_state: s,
                    sources: sources(snap, m),
                })
            })
            .collect();
        heuristic |= memberships.iter().any(|m| m.evidence_state == EvidenceState::ModelProposed);
        derived |= snap.is_derived_concept(a) || snap.is_derived_concept(b);
        let other_premises = snap.claim_targets(st.claim).len().saturating_sub(1);
        conj |= other_premises > 0 && l.way == Way::Converse;
        let raw = RawStep { from: st.from_decl, to: st.to_decl, claim: st.claim, rel: st.rel, way: l.way, state: st.dep_state, missing: 0 };
        let drill = drill_down.then(|| {
            let mut wctx = QueryCtx::new(snap, table, &engine.index, st.from_decl, st.to_decl, Vec::new(), ctx.w, ctx.state_mask);
            wctx.min_strength = 0;
            explain_path(&wctx, &RawPath { steps: vec![raw], kind: table.step_kind(st.rel, l.way) }, lang)
        });
        let spec = table.step(st.rel, l.way).expect("searched steps are walkable");
        steps.push(ConceptStep {
            from: snap.node_id(a).into(),
            to: snap.node_id(b).into(),
            from_label: snap.label(a, lang).into(),
            to_label: snap.label(b, lang).into(),
            from_derived: snap.is_derived_concept(a),
            to_derived: snap.is_derived_concept(b),
            relation: st.rel,
            direction: if l.way == Way::Forward { Direction::Forward } else { Direction::Inverse },
            evidence_state: st.state,
            authorizes: tr(lang,
                format!("a nivel de concepto ({} afirmaciones entre sus declaraciones): {}", st.count, spec.authorizes(lang)),
                format!("at concept level ({} claims between their declarations): {}", st.count, spec.authorizes(lang))),
            route_so_far: table.conclusion(kind).name.clone(),
            declaration_claims: st.count,
            counts,
            witness: Witness {
                claim_id: snap.claim_id(st.claim).into(),
                from: snap.node_id(st.from_decl).into(),
                to: snap.node_id(st.to_decl).into(),
                from_label: snap.label(st.from_decl, lang).into(),
                to_label: snap.label(st.to_decl, lang).into(),
                evidence_state: st.dep_state,
                other_premises,
                sources: sources(snap, st.claim),
                memberships,
            },
            drill_down: drill,
        });
    }
    let info = table.conclusion(kind);
    let strength = table.strength(kind);
    let mut caveats = vec![tr(lang,
        "Ruta a nivel de concepto: cada paso agrega afirmaciones entre declaraciones de los dos conceptos; su testigo es una de ellas.".into(),
        "Concept-level route: each step aggregates claims between declarations of the two concepts; its witness is one of them.".into())];
    if chain.len() > 1 {
        caveats.push(tr(lang,
            "Los testigos de pasos consecutivos pueden ser declaraciones distintas: no es una cadena de dependencias entre declaraciones.".into(),
            "The witnesses of consecutive steps may be different declarations: this is not a chain of dependencies between declarations.".into()));
    }
    if heuristic {
        caveats.push(tr(lang,
            "La pertenencia de alguna declaracion a su concepto es una asignacion heuristica (model_proposed), con puntuacion no calibrada.".into(),
            "Some declaration's membership in its concept is a heuristic assignment (model_proposed) with an uncalibrated score.".into()));
    }
    if derived {
        caveats.push(tr(lang,
            "Algun concepto es derivado de la jerarquia de modulos de Mathlib, no un concepto de Wikidata.".into(),
            "Some concept is derived from the Mathlib module hierarchy, not a Wikidata concept.".into()));
    }
    if conj {
        caveats.push(tr(lang,
            "Algun testigo usa ademas otras premisas: el concepto se usa para establecer el siguiente, pero no basta por si solo (R7).".into(),
            "Some witness also uses other premises: the concept is used to establish the next one but is not sufficient on its own (R7).".into()));
    }
    let last_g = labels[last as usize].g;
    ConceptRoute {
        score: last_g + ctx.terminal_cost(kind),
        conclusion: Conclusion {
            authorizes: tr(lang, format!("a nivel de concepto (agregado): {}", info.text(lang)), format!("at concept level (aggregated): {}", info.text(lang))),
            assumptions: Vec::new(),
            weakest_evidence: weakest_state,
            is_hypothesis: table.is_hypothetical(kind),
            kind: info.name.clone(),
            category: category(&info.family, info.mathematical),
            strength,
            mathematical: info.mathematical,
            complete: true,
            missing_premises: Vec::new(),
            caveats,
        },
        steps,
    }
}
