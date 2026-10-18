//! Query resolution, the k-route loop (penalty method) and the explanation of
//! every step, conclusion and abstention.

use ahash::AHashMap;
use atlas_core::{ClaimIx, Dir, Error, EvidenceState, NodeIx, RelationType, Result, Role, Snapshot};
use std::collections::BTreeMap;
use std::time::Instant;

use crate::algebra::{Conjunction, EMPTY, Kind, NONE, Way};
use crate::derive::{PROVE_DEPTH, Why};
use crate::search::{QueryCtx, RawPath, RawStep, Stats, Weights, search, search_capped};
use crate::{
    Conclusion, Direction, FoundPath, PathEngine, PathOptions, PathQuery, PathResponse, PremiseSupport, SearchStats,
    SourceRef, Step,
};

const MAX_HOPS: u32 = 12;
const MAX_K: usize = 10;
/// Claims listed per satisfied premise derivation.
const DERIVATION_CAP: usize = 12;
/// Relaxations for the raw (untyped) connection used to explain an abstention.
const RAW_BUDGET: usize = 4_000_000;
/// Expansions for each diagnosis search (relaxed rules, run only to explain an
/// abstention).  Without it a `min_strength = 0` search walks every citation.
/// Measured on the 1M-node synthetic graph: 25,000 cost 0.25-0.48 s per search
/// (~31 relaxations per expansion); 5,000 keeps an abstention near 0.1 s.  The
/// answer says when the diagnosis hit this bound.
const DIAGNOSE_EXPANSIONS: usize = 5_000;

fn tr(lang: &str, es: String, en: String) -> String {
    if lang == "en" { en } else { es }
}

/// Evidence order for "weakest": disputed < model_proposed < extracted < reviewed < formal.
fn evidence_rank(s: EvidenceState) -> u8 {
    match s {
        EvidenceState::FormallyLinked => 4,
        EvidenceState::EditoriallyReviewed => 3,
        EvidenceState::Extracted => 2,
        EvidenceState::ModelProposed => 1,
        _ => 0,
    }
}

pub fn run(engine: &PathEngine, q: &PathQuery, opts: &PathOptions) -> Result<PathResponse> {
    let t0 = Instant::now();
    let snap = engine.snapshot;
    if !(1..=MAX_HOPS).contains(&q.max_hops) {
        return Err(Error::BadRequest(format!("max_hops must be in 1..={MAX_HOPS}")));
    }
    if !(1..=MAX_K).contains(&opts.k) || opts.min_strength > 5 || opts.max_expansions == 0 {
        return Err(Error::BadRequest(format!("k must be in 1..={MAX_K}, min_strength in 0..=5, max_expansions > 0")));
    }
    if !(opts.max_stretch.is_finite() && opts.max_stretch >= 1.0) {
        return Err(Error::BadRequest("max_stretch must be finite and >= 1".into()));
    }
    if !(opts.alt_budget_factor.is_finite() && opts.alt_budget_factor >= 0.0) {
        return Err(Error::BadRequest("alt_budget_factor must be finite and >= 0".into()));
    }
    let resolve = |id: &str| snap.resolve(id).ok_or_else(|| Error::NotFound(format!("entity {id}")));
    let (s, t) = (resolve(&q.from)?, resolve(&q.to)?);
    if s == t {
        return Err(Error::BadRequest(format!("{} and {} are the same entity (after merges)", q.from, q.to)));
    }
    let known = opts.known.iter().map(|id| resolve(id)).collect::<Result<Vec<_>>>()?;
    let weights = engine.weights(q)?;
    let mut state_mask = atlas_core::CURRENT_STATES_MASK;
    if !q.include_disputed {
        state_mask &= !EvidenceState::Disputed.bit();
    }
    let mut ctx = QueryCtx::new(snap, &engine.table, &engine.index, s, t, known, Weights::from_map(&weights), state_mask);
    ctx.max_hops = q.max_hops as u8;
    ctx.min_strength = opts.min_strength;
    ctx.strict = opts.strict_conjunctions;
    ctx.bidirectional = opts.bidirectional;
    ctx.heuristic = opts.heuristic;
    ctx.max_expansions = opts.max_expansions;
    ctx.derive_budget = opts.derive_budget;

    let mut resp = PathResponse {
        from: snap.node_id(s).to_owned(),
        to: snap.node_id(t).to_owned(),
        objective: q.objective,
        weights,
        abstained: false,
        reason: None,
        abstention: None,
        paths: Vec::new(),
        exploration: None,
        composition_version: engine.table.version,
        stats: SearchStats {
            algorithm: match (opts.bidirectional, opts.heuristic) {
                (true, true) => "bidirectional-alt",
                (true, false) => "bidirectional-dijkstra",
                (false, true) => "alt-astar",
                (false, false) => "dijkstra",
            },
            runs: 0,
            expanded: 0,
            relaxed: 0,
            labels: 0,
            truncated: false,
            alternatives_capped: false,
            conjunctions_rejected: 0,
            derivable: None,
            derivable_complete: None,
            premise_proofs: 0,
            premise_micros: 0,
            micros: 0,
        },
    };
    let lang = q.lang.as_str();

    if snap.evidence(t).is_unknown() || snap.evidence(s).is_unknown() {
        let who = if snap.evidence(t).is_unknown() { t } else { s };
        abstain(&mut resp, "coverage_unknown", tr(lang,
            format!("{} no tiene afirmaciones extraidas: su cobertura es desconocida, no esta aislado.", snap.node_id(who)),
            format!("{} has no extracted claims: its coverage is unknown, not isolated.", snap.node_id(who))));
    } else if !engine.index.connected(s, t) {
        abstain(&mut resp, "disconnected", tr(lang,
            "Ninguna cadena de afirmaciones vigentes conecta estos dos nodos (componentes distintas).".into(),
            "No chain of current claims connects these two nodes (different components).".into()));
    } else {
        let mut total = Stats::default();
        let found = k_routes(&ctx, opts, &mut total, &mut resp.stats.runs);
        resp.stats.truncated = total.truncated;
        resp.stats.alternatives_capped = total.capped && found.len() < opts.k;
        resp.paths = found.iter().map(|p| explain_path(&ctx, p, lang)).collect();
        resp.paths.sort_by(|a, b| a.score.total_cmp(&b.score));
        if resp.paths.is_empty() {
            diagnose(&mut ctx, &mut resp, &total, lang);
        } else if total.truncated {
            for p in &mut resp.paths {
                p.conclusion.caveats.push(tr(lang,
                    "La busqueda agoto su presupuesto: puede haber rutas mejores.".into(),
                    "The search ran out of budget: better routes may exist.".into()));
            }
        }
        resp.stats.expanded = total.expanded;
        resp.stats.relaxed = total.relaxed;
        resp.stats.labels = total.labels;
        resp.stats.conjunctions_rejected = total.conj_rejected;
    }
    if let Some(d) = ctx.derivable_if_computed() {
        resp.stats.derivable = Some(d.size);
        resp.stats.derivable_complete = Some(d.complete);
        resp.stats.premise_proofs = d.backward_checks.get();
        resp.stats.premise_micros = d.nanos.get() / 1000;
    }
    resp.stats.micros = t0.elapsed().as_micros() as u64;
    Ok(resp)
}

fn abstain(resp: &mut PathResponse, code: &str, reason: String) {
    resp.abstained = true;
    resp.abstention = Some(code.to_owned());
    resp.reason = Some(reason);
}

/// Penalty method: after each route, its claims cost more; distinct routes only.
/// The first run is exact within `max_expansions`.  Each later run gets
/// `alt_budget_factor` times the first run's expansions (at least
/// `alt_min_expansions`): on dense graphs a penalized run explores 10-100x more
/// than the first one, and an alternative is worth less than a bounded tail.  A
/// later run cut by that budget keeps any valid route it found and marks the
/// alternatives as capped; it never casts doubt on the first route.
fn k_routes(ctx: &QueryCtx, opts: &PathOptions, total: &mut Stats, runs: &mut usize) -> Vec<RawPath> {
    let mut penalty: AHashMap<ClaimIx, f64> = AHashMap::new();
    let mut seen: AHashMap<Vec<ClaimIx>, u32> = AHashMap::new();
    let mut out = Vec::new();
    let mut cap = f64::INFINITY;
    let mut budget = ctx.max_expansions;
    for _ in 0..2 * opts.k {
        let first = *runs == 0;
        let (p, mut st) = search_capped(ctx, &penalty, cap, budget);
        *runs += 1;
        if std::env::var_os("ATLAS_PATHS_TRACE").is_some() {
            eprintln!("    run {}: expanded {} relaxed {} labels {} trunc {} -> {:?}", runs, st.expanded, st.relaxed, st.labels, st.truncated,
                p.as_ref().map(|p| (p.steps.len(), ctx.table.conclusion(p.kind).name.clone(), p.steps.iter().map(|s| (s.rel.as_str(), s.way as u8, s.claim)).collect::<Vec<_>>())));
        }
        let cut = !first && st.truncated;
        if cut {
            st.truncated = false;
            st.capped = true;
        }
        total.add(&st);
        let Some(p) = p else { break };
        let route: f64 = p.steps.iter().map(|s| ctx.step_terms(s).weighted(&ctx.w)).sum();
        if first {
            cap = opts.max_stretch * (route + ctx.terminal_cost(p.kind));
            let alt = (st.expanded as f64 * opts.alt_budget_factor) as usize;
            budget = budget.min(alt.max(opts.alt_min_expansions));
        }
        let claims: Vec<ClaimIx> = p.steps.iter().map(|s| s.claim).collect();
        let found_before = seen.get(&claims).copied().unwrap_or(0);
        // a route found again gets its cost added to each claim, doubled every time,
        // so it cannot keep winning when its rivals share some of its claims
        for step in &p.steps {
            let c = if found_before == 0 {
                ctx.step_terms(step).weighted(&ctx.w).max(0.1)
            } else {
                route.max(1.0) * f64::from(1u32 << found_before.min(16))
            };
            *penalty.entry(step.claim).or_insert(0.0) += c;
        }
        seen.insert(claims, found_before + 1);
        if found_before == 0 {
            out.push(p);
            if out.len() == opts.k {
                break;
            }
        }
        if st.truncated || cut {
            break;
        }
    }
    out
}

fn diagnose(ctx: &mut QueryCtx, resp: &mut PathResponse, total: &Stats, lang: &str) {
    if total.truncated {
        abstain(resp, "budget", tr(lang,
            format!("La busqueda agoto su presupuesto ({} estados) sin hallar una ruta autorizada; puede existir mas alla.", ctx.max_expansions),
            format!("The search ran out of budget ({} states) before finding an authorized route; one may exist beyond it.", ctx.max_expansions)));
        return;
    }
    let budget = ctx.max_expansions;
    ctx.max_expansions = budget.min(DIAGNOSE_EXPANSIONS);
    let mut cut = false;
    diagnose_bounded(ctx, resp, total, lang, &mut cut);
    ctx.max_expansions = budget;
    if cut && let Some(r) = &mut resp.reason {
        r.push_str(&tr(lang,
            format!(" (Diagnostico acotado a {DIAGNOSE_EXPANSIONS} estados por busqueda: puede haber una conexion debil o en disputa mas alla.)"),
            format!(" (Diagnosis bounded to {DIAGNOSE_EXPANSIONS} states per search: a weak or disputed connection may exist beyond it.)")));
    }
}

/// One diagnosis search with relaxed rules; records whether it hit its budget.
fn relaxed_search(ctx: &QueryCtx, cut: &mut bool) -> Option<RawPath> {
    let t0 = Instant::now();
    let (p, st) = search(ctx, &AHashMap::new());
    if std::env::var_os("ATLAS_PATHS_TRACE").is_some() {
        eprintln!("    diagnosis search: {:.1} ms, expanded {} relaxed {}", t0.elapsed().as_secs_f64() * 1e3, st.expanded, st.relaxed);
    }
    *cut |= st.truncated;
    p
}

fn diagnose_bounded(ctx: &mut QueryCtx, resp: &mut PathResponse, total: &Stats, lang: &str, cut: &mut bool) {
    if ctx.strict && total.conj_rejected > 0 {
        ctx.strict = false;
        let p = relaxed_search(ctx, cut);
        ctx.strict = true;
        if let Some(p) = p {
            let e = explain_path(ctx, &p, lang);
            let missing = e.conclusion.missing_premises.join(", ");
            abstain(resp, "strict_conjunction", tr(lang,
                format!("Solo hay rutas con premisas conjuntas sin satisfacer ({missing}); en modo estricto no cuentan (R7)."),
                format!("Only routes with unsatisfied conjunctive premises exist ({missing}); strict mode does not count them (R7).")));
            resp.exploration = Some(e);
            return;
        }
    }
    if ctx.min_strength > 0 {
        let min = ctx.min_strength;
        ctx.min_strength = 0;
        let p = relaxed_search(ctx, cut);
        ctx.min_strength = min;
        if let Some(p) = p {
            let e = explain_path(ctx, &p, lang);
            abstain(resp, "only_weak", tr(lang,
                format!("Solo hay una conexion de tipo «{}» (fuerza {} < {min}): no autoriza una conclusion.", e.conclusion.authorizes, e.conclusion.strength),
                format!("Only a «{}» connection exists (strength {} < {min}): it authorizes no conclusion.", e.conclusion.authorizes, e.conclusion.strength)));
            resp.exploration = Some(e);
            return;
        }
    }
    if ctx.state_mask & EvidenceState::Disputed.bit() == 0 {
        ctx.state_mask |= EvidenceState::Disputed.bit();
        let p = relaxed_search(ctx, cut);
        ctx.state_mask &= !EvidenceState::Disputed.bit();
        if let Some(p) = p {
            let disputed: Vec<&str> = p
                .steps
                .iter()
                .filter(|s| s.state == EvidenceState::Disputed)
                .map(|s| ctx.snap.claim_id(s.claim))
                .collect();
            let list = disputed.join(", ");
            abstain(resp, "disputed_only", tr(lang,
                format!("Solo hay rutas que usan afirmaciones en disputa ({list}); se usan solo si se pide (include_disputed, R8)."),
                format!("Only routes through disputed claims exist ({list}); they are used only on request (include_disputed, R8).")));
            resp.exploration = Some(explain_path(ctx, &p, lang));
            return;
        }
    }
    let t_raw = Instant::now();
    let raw = raw_route(ctx, false, cut).or_else(|| raw_route(ctx, true, cut));
    if std::env::var_os("ATLAS_PATHS_TRACE").is_some() {
        eprintln!("    raw route: {:.1} ms", t_raw.elapsed().as_secs_f64() * 1e3);
    }
    match raw {
        Some(p) => {
            let (blocked_at, prefix) = first_block(ctx, &p);
            let e = explain_path(ctx, &p, lang);
            let step = &p.steps[blocked_at];
            let word = format!("{}{}", step.rel, if step.way == Way::Converse { "^-1" } else { "" });
            let pre = ctx.table.conclusion(prefix).name.clone();
            let rule = block_rule(ctx, prefix, ctx.table.step_kind(step.rel, step.way));
            abstain(resp, "unauthorized", tr(lang,
                format!("Hay una conexion de {} saltos, pero «{pre}» seguido de {word} (salto {}) no autoriza nada ({rule}). Se muestra solo como exploracion.", p.steps.len(), blocked_at + 1),
                format!("A {}-hop connection exists, but «{pre}» followed by {word} (step {}) authorizes nothing ({rule}). Shown as exploration only.", p.steps.len(), blocked_at + 1)));
            resp.exploration = Some(e);
        }
        None if *cut => {
            *cut = false;
            abstain(resp, "budget", tr(lang,
                format!("No se hallo ruta autorizada y el diagnostico agoto su presupuesto ({DIAGNOSE_EXPANSIONS} estados, {RAW_BUDGET} aristas) sin hallar ninguna conexion; puede existir mas alla."),
                format!("No authorized route was found and the diagnosis ran out of budget ({DIAGNOSE_EXPANSIONS} states, {RAW_BUDGET} edges) without finding any connection; one may exist beyond it.")));
        }
        None => {
            let lb = ctx.index.hop_lb(ctx.s, ctx.t);
            abstain(resp, "hop_limit", tr(lang,
                format!("No hay conexion de a lo sumo {} saltos (cota inferior medida: {lb}).", ctx.max_hops),
                format!("No connection within {} hops (measured lower bound: {lb}).", ctx.max_hops)));
        }
    }
}

fn block_rule(ctx: &QueryCtx, prefix: Kind, step: Kind) -> &'static str {
    let t = ctx.table;
    if step == NONE {
        "direction: this relation is not walked against its stored direction"
    } else if t.is_hypothetical(prefix) && t.is_hypothetical(step) {
        "R10"
    } else if [prefix, step].iter().any(|k| t.conclusion(*k).family == "learning") {
        "R5"
    } else if [prefix, step].iter().any(|k| t.strength(*k) == 0) {
        "R4"
    } else if ctx.table.compose(prefix, step) != NONE {
        "min_strength"
    } else {
        "mixing"
    }
}

/// Index of the first step whose composition fails (or drops below min_strength), and the prefix kind.
fn first_block(ctx: &QueryCtx, p: &RawPath) -> (usize, Kind) {
    let mut k = EMPTY;
    for (i, s) in p.steps.iter().enumerate() {
        let n = ctx.table.compose(k, ctx.table.step_kind(s.rel, s.way));
        if n == NONE || ctx.table.strength(n) < ctx.min_strength {
            return (i, k);
        }
        k = n;
    }
    (p.steps.len() - 1, k)
}

type Par = (NodeIx, ClaimIx, RelationType, Way, u8);
const UNSEEN: u32 = u32::MAX;

/// node -> parent step for one side of `raw_route`: dense slots pooled per thread
/// and reset through the entries, so a diagnosis pays for the nodes it reaches,
/// not for the size of the graph (a hash map here cost seconds on 1M nodes).
struct ParMap {
    slot: Vec<u32>,
    ent: Vec<(NodeIx, Option<Par>)>,
}

thread_local! {
    static PARS: std::cell::RefCell<Vec<ParMap>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl ParMap {
    fn take(n: usize) -> ParMap {
        let mut m = PARS.with(|p| p.borrow_mut().pop()).unwrap_or(ParMap { slot: Vec::new(), ent: Vec::new() });
        if m.slot.len() != n {
            m.slot = vec![UNSEEN; n];
            m.ent.clear();
        }
        m
    }

    fn recycle(mut self) {
        for &(x, _) in &self.ent {
            self.slot[x as usize] = UNSEEN;
        }
        self.ent.clear();
        PARS.with(|p| {
            let mut p = p.borrow_mut();
            if p.len() < 2 {
                p.push(self);
            }
        });
    }

    #[inline]
    fn get(&self, x: &NodeIx) -> Option<&Option<Par>> {
        let i = self.slot[*x as usize];
        (i != UNSEEN).then(|| &self.ent[i as usize].1)
    }

    #[inline]
    fn contains_key(&self, x: &NodeIx) -> bool {
        self.slot[*x as usize] != UNSEEN
    }

    /// First insertion of `x` only (callers check `contains_key`).
    #[inline]
    fn insert(&mut self, x: NodeIx, p: Option<Par>) {
        self.slot[x as usize] = self.ent.len() as u32;
        self.ent.push((x, p));
    }
}

/// Fewest-hop connection ignoring composition (bidirectional BFS) over walkable
/// steps, or over every current edge in either direction when `any_direction`.
fn raw_route(ctx: &QueryCtx, any_direction: bool, cut: &mut bool) -> Option<RawPath> {
    let n = ctx.snap.num_nodes() as usize;
    let mut par = [ParMap::take(n), ParMap::take(n)];
    let r = raw_bfs(ctx, any_direction, cut, &mut par);
    let [a, b] = par;
    a.recycle();
    b.recycle();
    r
}

fn raw_bfs(ctx: &QueryCtx, any_direction: bool, cut: &mut bool, par: &mut [ParMap; 2]) -> Option<RawPath> {
    let snap = ctx.snap;
    let (out, inn) = (snap.csr(Dir::Out), snap.csr(Dir::In));
    par[0].insert(ctx.s, None);
    par[1].insert(ctx.t, None);
    let mut frontier = [vec![ctx.s], vec![ctx.t]];
    let mut depth = [0u32, 0u32];
    let mut budget = RAW_BUDGET;
    while depth[0] + depth[1] < ctx.max_hops as u32 && !frontier[0].is_empty() && !frontier[1].is_empty() {
        let side = if frontier[0].len() <= frontier[1].len() { 0 } else { 1 };
        let mut next = Vec::new();
        for &x in &frontier[side] {
            let lists = if side == 0 {
                [(out.edges(x), Way::Forward), (inn.edges(x), Way::Converse)]
            } else {
                [(inn.edges(x), Way::Forward), (out.edges(x), Way::Converse)]
            };
            for (e, way) in lists {
                budget = budget.saturating_sub(e.len());
                for i in 0..e.len() {
                    let rel = RelationType::from_code(e.rel[i]).expect("verified snapshot");
                    if ctx.state_mask & (1 << e.state[i]) == 0 || (!any_direction && ctx.table.step(rel, way).is_none()) {
                        continue;
                    }
                    let y = e.nbr[i];
                    if par[side].contains_key(&y) {
                        continue;
                    }
                    par[side].insert(y, Some((x, e.claim[i], rel, way, e.state[i])));
                    if par[1 - side].contains_key(&y) {
                        return Some(join(par, y));
                    }
                    next.push(y);
                }
            }
            if budget == 0 {
                *cut = true;
                return None;
            }
        }
        frontier[side] = next;
        depth[side] += 1;
    }
    None
}

fn join(par: &[ParMap; 2], meet: NodeIx) -> RawPath {
    let mk = |from, to, (_, claim, rel, way, state): Par| RawStep {
        from,
        to,
        claim,
        rel,
        way,
        state: EvidenceState::from_code(state).expect("verified snapshot"),
        missing: 0,
    };
    let mut steps = Vec::new();
    let mut x = meet;
    while let Some(Some(p)) = par[0].get(&x) {
        steps.push(mk(p.0, x, *p));
        x = p.0;
    }
    steps.reverse();
    let mut x = meet;
    while let Some(Some(p)) = par[1].get(&x) {
        steps.push(mk(x, p.0, *p));
        x = p.0;
    }
    RawPath { steps, kind: NONE }
}

// ------------------------------------------------------------------ one route

fn entity_text(snap: &Snapshot, n: NodeIx, lang: &str) -> String {
    format!("{} ({})", snap.label(n, lang), snap.node_id(n))
}

fn explain_path(ctx: &QueryCtx, p: &RawPath, lang: &str) -> FoundPath {
    let (snap, table) = (ctx.snap, ctx.table);
    let mut steps = Vec::with_capacity(p.steps.len());
    let mut terms: BTreeMap<String, f64> = BTreeMap::new();
    let mut score = 0.0;
    let mut kind = EMPTY;
    let mut assumptions: Vec<String> = Vec::new();
    let mut missing_all: Vec<String> = Vec::new();
    let mut caveats: Vec<String> = Vec::new();
    let mut weakest = EvidenceState::FormallyLinked;
    let push_unique = |v: &mut Vec<String>, s: String| {
        if !v.contains(&s) {
            v.push(s);
        }
    };
    for st in &p.steps {
        let spec = table.step(st.rel, st.way);
        let c = st.claim;
        kind = table.compose(kind, spec.map_or(NONE, |s| s.kind));
        let t = ctx.step_terms(st);
        score += t.weighted(&ctx.w);
        for (name, v) in [
            ("length", t.length),
            ("evidence", t.evidence),
            ("formal", t.formal),
            ("new", t.new),
            ("missing_premise", t.missing_premise),
            ("diversity", t.diversity),
            ("intuition", t.intuition),
        ] {
            *terms.entry(name.into()).or_insert(0.0) += v;
        }
        if evidence_rank(st.state) < evidence_rank(weakest) {
            weakest = st.state;
        }
        if st.state == EvidenceState::Disputed {
            push_unique(&mut caveats, tr(lang,
                format!("La afirmacion {} esta en disputa (se uso porque se pidio).", snap.claim_id(c)),
                format!("Claim {} is disputed (used because it was requested).", snap.claim_id(c))));
        }
        let mut step_asm: Vec<String> = snap.claim_assumptions(c).map(str::to_owned).collect();
        let (mut via, mut context) = (Vec::new(), Vec::new());
        for (role, n) in snap.claim_context(c) {
            match role {
                Role::Assumption => step_asm.push(entity_text(snap, n, lang)),
                Role::Via => via.push(snap.node_id(n).to_owned()),
                Role::Context => context.push(snap.node_id(n).to_owned()),
                _ => {}
            }
        }
        let validity = snap.claim_validity_domain(c).map(str::to_owned);
        if spec.is_some_and(|s| s.assumptions_union) {
            for a in &step_asm {
                push_unique(&mut assumptions, a.clone());
            }
            if let Some(v) = &validity {
                push_unique(&mut assumptions, tr(lang, format!("valido en: {v}"), format!("valid in: {v}")));
            }
        }
        let conj = snap.conjunct_count(c) > 1 && st.rel.spec().allows_premises;
        let premises: Vec<NodeIx> = if conj { snap.claim_targets(c).to_vec() } else { Vec::new() };
        let mut missing = Vec::new();
        let mut satisfied = Vec::new();
        if conj && spec.is_some_and(|s| s.conjunction == Conjunction::RequireAll) {
            let d = ctx.derivable();
            for &pm in &premises {
                if pm == st.from {
                    continue;
                }
                if ctx.known.contains(&pm) {
                    satisfied.push(PremiseSupport { premise: snap.node_id(pm).into(), how: "known", claims: vec![], truncated: false });
                } else if let Some(why) = d.why(pm) {
                    let (claims, truncated) = d.derivation(pm, DERIVATION_CAP);
                    let how = if why == Why::Start { "start" } else { "derived" };
                    let claims = claims.iter().map(|&x| snap.claim_id(x).to_owned()).collect();
                    satisfied.push(PremiseSupport { premise: snap.node_id(pm).into(), how, claims, truncated });
                } else {
                    missing.push(snap.node_id(pm).to_owned());
                    push_unique(&mut missing_all, snap.node_id(pm).to_owned());
                    push_unique(&mut assumptions, tr(lang,
                        format!("premisa no satisfecha: {}", entity_text(snap, pm, lang)),
                        format!("unsatisfied premise: {}", entity_text(snap, pm, lang))));
                }
            }
            if !missing.is_empty() && !d.is_exact() {
                push_unique(&mut caveats, tr(lang,
                    format!("Lo derivable desde el origen se calculo con cotas (cierre hacia delante parcial, derivaciones hacia atras de a lo sumo {PROVE_DEPTH} niveles): alguna premisa faltante podria ser derivable."),
                    format!("What the start establishes was computed under bounds (partial forward closure, backward derivations of at most {PROVE_DEPTH} levels): a missing premise might be derivable.")));
            }
        }
        let alternatives = alternatives(ctx, c);
        if st.rel == RelationType::DependsOnLogically && st.way == Way::Forward && !alternatives.is_empty() {
            // R11: necessity only if every alternative proof uses the same premise
            let avoid: Vec<&str> = snap
                .claim_group(c)
                .map(|g| snap.group_claims(g))
                .unwrap_or(&[])
                .iter()
                .filter(|&&a| a != c && ctx.state_mask & snap.claim_state(a).bit() != 0)
                .filter(|&&a| !snap.claim_targets(a).contains(&st.to))
                .map(|&a| snap.claim_id(a))
                .collect();
            if !avoid.is_empty() {
                push_unique(&mut caveats, tr(lang,
                    format!("{} no es necesaria para {}: la alternativa {} no la usa (R11).", snap.node_id(st.to), snap.node_id(st.from), avoid.join(", ")),
                    format!("{} is not necessary for {}: alternative {} does not use it (R11).", snap.node_id(st.to), snap.node_id(st.from), avoid.join(", "))));
            }
        }
        steps.push(Step {
            claim_id: snap.claim_id(c).to_owned(),
            relation: st.rel,
            from: snap.node_id(st.from).to_owned(),
            to: snap.node_id(st.to).to_owned(),
            from_label: snap.label(st.from, lang).to_owned(),
            to_label: snap.label(st.to, lang).to_owned(),
            direction: if st.way == Way::Forward { Direction::Forward } else { Direction::Inverse },
            evidence_state: st.state,
            authorizes: match spec {
                Some(s) => s.authorizes(lang).to_owned(),
                None => tr(lang, "no se recorre en este sentido: no autoriza nada".into(), "not walked in this direction: authorizes nothing".into()),
            },
            route_so_far: table.conclusion(kind).name.clone(),
            assumptions: step_asm,
            premises: premises.iter().map(|&n| snap.node_id(n).to_owned()).collect(),
            premises_missing: missing,
            premises_satisfied: satisfied,
            via,
            context,
            validity_domain: validity,
            alternatives,
            sources: snap
                .claim_provenance(c)
                .map(|p| SourceRef { source_id: p.source_id.into(), locator: p.locator.into(), license: p.license.into() })
                .collect(),
        });
    }
    let info = table.conclusion(kind);
    let strength = if kind == EMPTY { 0 } else { table.strength(kind) };
    score += ctx.terminal_cost(kind);
    terms.insert("conclusion".into(), (5 - strength) as f64);
    if !missing_all.is_empty() {
        caveats.push(tr(lang,
            "Ruta incompleta: un paso conjuntivo tiene premisas sin satisfacer; la conclusion vale solo si se suponen (R7).".into(),
            "Incomplete route: a conjunctive step has unsatisfied premises; the conclusion holds only if they are assumed (R7).".into()));
    }
    FoundPath {
        score,
        cost_terms: terms,
        conclusion: Conclusion {
            authorizes: info.text(lang).to_owned(),
            assumptions,
            weakest_evidence: weakest,
            is_hypothesis: table.is_hypothetical(kind),
            kind: info.name.clone(),
            strength,
            mathematical: info.mathematical && kind != NONE,
            complete: missing_all.is_empty(),
            missing_premises: missing_all,
            caveats,
        },
        steps,
    }
}

fn alternatives(ctx: &QueryCtx, c: ClaimIx) -> Vec<String> {
    let snap = ctx.snap;
    let Some(g) = snap.claim_group(c) else { return Vec::new() };
    snap.group_claims(g)
        .iter()
        .filter(|&&a| a != c && ctx.state_mask & snap.claim_state(a).bit() != 0)
        .map(|&a| snap.claim_id(a).to_owned())
        .collect()
}
