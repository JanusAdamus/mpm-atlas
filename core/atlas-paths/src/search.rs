//! Typed route search: bidirectional, label-setting, A* with ALT potentials.
//!
//! The state is (node, kind): the kind is the conclusion of the route so far in
//! the composition monoid, so the search runs on the product of the graph with
//! the algebra's automaton and never extends a route that authorizes nothing.
//! Strength is monotone non-increasing (checked on load), so a prefix weaker than
//! `min_strength` is pruned too.  Hop limits make it a resource-constrained search:
//! each (node, kind) keeps a Pareto set of (cost, hops) labels.
//!
//! Bidirectional: the forward side grows prefixes from `s` (kind = prefix o step),
//! the backward side grows suffixes into `t` (kind = step o suffix); they meet at a
//! node when prefix o suffix is authorized.  Associativity of the table is what
//! makes that splice equal to the left fold.  Potentials are the average of the
//! ALT bounds to `t` and from `s` (consistent for both sides); the stop rule is the
//! classic top_f + top_b >= mu on reduced costs.  Keys also carry a lower bound on
//! the conclusion term from the label's kind (`kind_bound`).  Conjunctive steps are
//! evaluated lazily: their premises are decided when the label is popped, not when
//! it is generated (`decide`).  Unidirectional mode keeps the
//! backward side as a single root label at `t`, which turns this into plain A*.

use ahash::{AHashMap, AHashSet};
use atlas_core::{ClaimIx, Csr, Dir, EvidenceState, NodeIx, RelationType, Snapshot};
use std::cell::OnceCell;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::algebra::{CompositionTable, Conjunction, EMPTY, Kind, MAX_STRENGTH, NONE, Way};
use crate::derive::{Derivable, closure};
use crate::index::GraphIndex;

/// Every step costs at least this much, so zero-weight objectives still prefer
/// simple routes and the label sets stay finite.
const MIN_STEP_COST: f64 = 1e-6;
const NIL: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Default)]
pub struct Weights {
    pub length: f64,
    pub evidence: f64,
    pub formal: f64,
    pub new: f64,
    pub missing_premise: f64,
    pub diversity: f64,
    pub intuition: f64,
    pub conclusion: f64,
}

impl Weights {
    pub fn from_map(m: &std::collections::BTreeMap<String, f64>) -> Weights {
        let g = |k: &str| m.get(k).copied().unwrap_or(0.0);
        Weights {
            length: g("length"),
            evidence: g("evidence"),
            formal: g("formal"),
            new: g("new"),
            missing_premise: g("missing_premise"),
            diversity: g("diversity"),
            intuition: g("intuition"),
            conclusion: g("conclusion"),
        }
    }
}

/// Cost terms of one step, before weighting (the explanation shows them).
#[derive(Clone, Copy, Debug, Default)]
pub struct Terms {
    pub length: f64,
    pub evidence: f64,
    pub formal: f64,
    pub new: f64,
    pub missing_premise: f64,
    pub diversity: f64,
    pub intuition: f64,
}

impl Terms {
    pub fn weighted(&self, w: &Weights) -> f64 {
        let c = w.length * self.length
            + w.evidence * self.evidence
            + w.formal * self.formal
            + w.new * self.new
            + w.missing_premise * self.missing_premise
            + w.diversity * self.diversity
            + w.intuition * self.intuition;
        c.max(MIN_STEP_COST)
    }
}

/// One resolved query, shared by the k runs of the penalty method.
pub struct QueryCtx<'a> {
    pub snap: &'a Snapshot,
    pub table: &'a CompositionTable,
    pub index: &'a GraphIndex,
    pub s: NodeIx,
    pub t: NodeIx,
    pub max_hops: u8,
    pub min_strength: u8,
    /// Bitmask of `EvidenceState::bit()` the route may use (R8).
    pub state_mask: u32,
    pub strict: bool,
    pub known: AHashSet<NodeIx>,
    pub known_list: Vec<NodeIx>,
    pub w: Weights,
    pub bidirectional: bool,
    pub heuristic: bool,
    pub max_expansions: usize,
    pub derive_budget: usize,
    out: Csr<'a>,
    inn: Csr<'a>,
    min_step: f64,
    derivable: OnceCell<Derivable<'a>>,
}

#[allow(clippy::too_many_arguments)]
impl<'a> QueryCtx<'a> {
    pub fn new(
        snap: &'a Snapshot,
        table: &'a CompositionTable,
        index: &'a GraphIndex,
        s: NodeIx,
        t: NodeIx,
        known_list: Vec<NodeIx>,
        w: Weights,
        state_mask: u32,
    ) -> QueryCtx<'a> {
        let min_pen = EvidenceState::ALL
            .iter()
            .filter(|st| state_mask & st.bit() != 0)
            .map(|st| table.evidence_penalty(*st))
            .fold(f64::INFINITY, f64::min);
        let min_intuition = RelationType::ALL
            .iter()
            .flat_map(|r| [table.step(*r, Way::Forward), table.step(*r, Way::Converse)])
            .flatten()
            .map(|s| s.intuition)
            .fold(f64::INFINITY, f64::min);
        let min_step =
            (w.length + w.evidence * min_pen.min(1e9) + w.intuition * min_intuition.min(1e9)).max(MIN_STEP_COST);
        QueryCtx {
            snap,
            table,
            index,
            s,
            t,
            max_hops: 6,
            min_strength: 1,
            state_mask,
            strict: false,
            known: known_list.iter().copied().collect(),
            known_list,
            w,
            bidirectional: true,
            heuristic: true,
            max_expansions: 1_000_000,
            derive_budget: 200_000,
            out: snap.csr(Dir::Out),
            inn: snap.csr(Dir::In),
            min_step,
            derivable: OnceCell::new(),
        }
    }

    /// The set of nodes established from `s` and `known` (computed on first use).
    pub fn derivable(&self) -> &Derivable<'a> {
        self.derivable
            .get_or_init(|| closure(self.snap, self.s, &self.known_list, self.state_mask, self.derive_budget))
    }

    pub fn derivable_if_computed(&self) -> Option<&Derivable<'a>> {
        self.derivable.get()
    }

    /// Premises of a conjunctive claim, other than `entered_from`, that are neither
    /// known nor established from the start.
    pub fn missing_premises(&self, c: ClaimIx, entered_from: NodeIx) -> impl Iterator<Item = NodeIx> + '_ {
        self.snap
            .claim_targets(c)
            .iter()
            .copied()
            .filter(move |&p| p != entered_from && !self.known.contains(&p) && !self.derivable().contains(p))
    }

    pub fn step_terms(&self, step: &RawStep) -> Terms {
        // exploration routes may hold a step that cannot be walked (no spec): no prior
        let intuition = self.table.step(step.rel, step.way).map_or(0.0, |s| s.intuition);
        let d = match (self.snap.area(step.from), self.snap.area(step.to)) {
            (Some(a), Some(b)) if a == b => 1.0,
            (Some(_), Some(_)) => 0.0,
            _ => 0.5,
        };
        let fresh = step.to != self.t && step.to != self.s && !self.known.contains(&step.to);
        Terms {
            length: 1.0,
            evidence: self.table.evidence_penalty(step.state),
            formal: (step.state != EvidenceState::FormallyLinked) as u8 as f64,
            new: fresh as u8 as f64 + step.missing as f64,
            missing_premise: step.missing as f64,
            diversity: d,
            intuition,
        }
    }

    /// `step_terms(step).weighted(w)` without the terms whose weight is zero.
    #[inline]
    pub fn step_cost(&self, step: &RawStep) -> f64 {
        let w = &self.w;
        let mut c = w.length;
        if w.evidence != 0.0 {
            c += w.evidence * self.table.evidence_penalty(step.state);
        }
        if w.formal != 0.0 && step.state != EvidenceState::FormallyLinked {
            c += w.formal;
        }
        if w.new != 0.0 {
            let fresh = step.to != self.t && step.to != self.s && (self.known.is_empty() || !self.known.contains(&step.to));
            c += w.new * (fresh as u8 as f64 + step.missing as f64);
        }
        c += w.missing_premise * step.missing as f64;
        if w.diversity != 0.0 {
            c += w.diversity * match (self.snap.area(step.from), self.snap.area(step.to)) {
                (Some(a), Some(b)) if a == b => 1.0,
                (Some(_), Some(_)) => 0.0,
                _ => 0.5,
            };
        }
        if w.intuition != 0.0 {
            c += w.intuition * self.table.step(step.rel, step.way).map_or(0.0, |s| s.intuition);
        }
        c.max(MIN_STEP_COST)
    }

    /// Lower bound on the cost of any single step under these weights.
    pub fn min_step(&self) -> f64 {
        self.min_step
    }

    pub fn terminal_cost(&self, k: Kind) -> f64 {
        self.w.conclusion * (MAX_STRENGTH - self.table.strength(k).min(MAX_STRENGTH)) as f64
    }

    /// Lower bound, from a partial route's kind, on the conclusion term of any route
    /// that extends it: strength never increases along a route (checked on load), so
    /// `terminal_cost(kind)` only grows.  Bidirectional: each side claims half, so the
    /// two keys of a split route still add up to at most its cost.
    #[inline]
    fn kind_bound(&self, k: Kind) -> f64 {
        if !self.heuristic || self.w.conclusion == 0.0 {
            return 0.0;
        }
        let c = self.terminal_cost(k);
        if self.bidirectional { 0.5 * c } else { c }
    }

    /// Forward potential p_f(v); the backward side uses -p_f.
    #[inline]
    fn potential(&self, v: NodeIx) -> f64 {
        let (to_t, from_s) = self.index.hop_lb2(v, self.t, self.s, self.min_strength >= 1);
        self.potential_from(to_t, from_s)
    }

    /// `potential` from the hop bounds v -> t and s -> v.
    #[inline]
    fn potential_from(&self, to_t: u32, from_s: u32) -> f64 {
        if !self.heuristic {
            return 0.0;
        }
        let to_t = self.min_step * to_t as f64;
        if self.bidirectional { 0.5 * (to_t - self.min_step * from_s as f64) } else { to_t }
    }
}

/// One step of a found route, in route order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawStep {
    pub from: NodeIx,
    pub to: NodeIx,
    pub claim: ClaimIx,
    pub rel: RelationType,
    pub way: Way,
    pub state: EvidenceState,
    pub missing: u16,
}

#[derive(Clone, Debug)]
pub struct RawPath {
    pub steps: Vec<RawStep>,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub expanded: usize,
    pub relaxed: usize,
    pub labels: usize,
    pub truncated: bool,
    pub conj_rejected: usize,
    pub derive_scanned: usize,
    /// Distinct nodes reached (both sides).
    pub nodes: usize,
    /// The run stopped at its cost cap (alternative routes, `max_stretch`).
    pub capped: bool,
}

impl Stats {
    pub fn add(&mut self, o: &Stats) {
        self.expanded += o.expanded;
        self.relaxed += o.relaxed;
        self.labels += o.labels;
        self.truncated |= o.truncated;
        self.capped |= o.capped;
        self.conj_rejected += o.conj_rejected;
        self.nodes += o.nodes;
    }
}

#[derive(Clone, Copy)]
struct Label {
    node: NodeIx,
    parent: u32,
    next_at_node: u32,
    claim: ClaimIx,
    g: f64,
    kind: Kind,
    hops: u8,
    rel: u8,
    way: u8,
    state: u8,
    missing: u16,
    alive: bool,
    /// Conjunctive step whose premises are not decided yet (lazy evaluation): `g`
    /// is a lower bound.  Decided when popped; never dominates, never meets.
    pending: bool,
}

#[derive(Clone, Copy)]
struct Item {
    key: f64,
    idx: u32,
}

impl PartialEq for Item {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        // min-heap on key, then FIFO-ish by index for determinism
        o.key.total_cmp(&self.key).then(o.idx.cmp(&self.idx))
    }
}

struct Side {
    forward: bool,
    labels: Vec<Label>,
    /// Newest label per node (dense, NIL = none), pooled per thread.
    head: Vec<u32>,
    touched: Vec<NodeIx>,
    heap: BinaryHeap<Item>,
}

thread_local! {
    static SIDES: std::cell::RefCell<Vec<Side>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl Side {
    fn new(forward: bool, root: NodeIx, nodes: usize) -> Side {
        let mut s = SIDES.with(|p| p.borrow_mut().pop()).unwrap_or_else(|| Side {
            forward,
            labels: Vec::new(),
            head: Vec::new(),
            touched: Vec::new(),
            heap: BinaryHeap::new(),
        });
        if s.head.len() != nodes {
            s.head = vec![NIL; nodes];
            s.touched.clear();
        }
        s.forward = forward;
        s.labels.push(Label {
            node: root,
            parent: NIL,
            next_at_node: NIL,
            claim: NIL,
            g: 0.0,
            kind: EMPTY,
            hops: 0,
            rel: 0,
            way: 0,
            state: 0,
            missing: 0,
            alive: true,
            pending: false,
        });
        s.head[root as usize] = 0;
        s.touched.push(root);
        s
    }

    /// Reset (cost: the nodes touched) and return the buffers to the pool.
    fn recycle(mut self) {
        for &n in &self.touched {
            self.head[n as usize] = NIL;
        }
        self.touched.clear();
        self.labels.clear();
        self.heap.clear();
        SIDES.with(|p| {
            let mut p = p.borrow_mut();
            if p.len() < 4 {
                p.push(self);
            }
        });
    }

    #[inline]
    fn head(&self, n: NodeIx) -> u32 {
        self.head[n as usize]
    }

    fn top(&mut self) -> Option<f64> {
        while let Some(it) = self.heap.peek() {
            if self.labels[it.idx as usize].alive {
                return Some(it.key);
            }
            self.heap.pop();
        }
        None
    }

    /// Is `l` subsumed by a decided label at its node other than `except`?  Kills
    /// the labels `l` subsumes when `l` is decided (a pending cost is only a lower
    /// bound, so a pending label neither subsumes nor kills).
    fn settle_dominance(&mut self, l: &Label, except: u32, table: &CompositionTable, min_strength: u8) -> bool {
        let prefix = self.forward;
        let mut i = self.head(l.node);
        while i != NIL {
            let o = &mut self.labels[i as usize];
            if o.alive && i != except {
                if !o.pending && o.g <= l.g && o.hops <= l.hops && table.dominates(min_strength, prefix, o.kind, l.kind) {
                    return true;
                }
                if !l.pending && l.g <= o.g && l.hops <= o.hops && table.dominates(min_strength, prefix, l.kind, o.kind) {
                    o.alive = false;
                }
            }
            i = o.next_at_node;
        }
        false
    }

    /// Insert unless a label at the node subsumes it (kind dominates, cost and hops
    /// no worse); kill the labels the new one subsumes.
    fn insert(&mut self, l: Label, key: f64, table: &CompositionTable, min_strength: u8) -> Option<u32> {
        if self.settle_dominance(&l, NIL, table, min_strength) {
            return None;
        }
        let idx = self.labels.len() as u32;
        let head = &mut self.head[l.node as usize];
        if *head == NIL {
            self.touched.push(l.node);
        }
        let mut l = l;
        l.next_at_node = *head;
        *head = idx;
        self.labels.push(l);
        self.heap.push(Item { key, idx });
        Some(idx)
    }

    fn chain_nodes(&self, mut i: u32, out: &mut Vec<NodeIx>) {
        while i != NIL {
            let l = &self.labels[i as usize];
            out.push(l.node);
            i = l.parent;
        }
    }

    /// Steps from the root to label `i` (forward side) or from `i` to the root (backward).
    fn steps(&self, mut i: u32) -> Vec<RawStep> {
        let mut v = Vec::new();
        while self.labels[i as usize].parent != NIL {
            let l = self.labels[i as usize];
            let p = self.labels[l.parent as usize].node;
            let (from, to) = if self.forward { (p, l.node) } else { (l.node, p) };
            v.push(RawStep {
                from,
                to,
                claim: l.claim,
                rel: RelationType::from_code(l.rel).expect("stored from a valid relation"),
                way: if l.way == 0 { Way::Forward } else { Way::Converse },
                state: EvidenceState::from_code(l.state).expect("stored from a valid state"),
                missing: l.missing,
            });
            i = l.parent;
        }
        if self.forward {
            v.reverse();
        }
        v
    }
}

struct Best {
    total: f64,
    f: u32,
    b: u32,
    kind: Kind,
}

/// Per run: for each side, way and kind, the relations (bit = code) whose step
/// composes into an authorized kind of at least `min_strength`.  Lets a hub skip
/// whole relation runs of its sorted edge list.
struct Allowed {
    kinds: usize,
    mask: Vec<u32>,
}

impl Allowed {
    fn new(ctx: &QueryCtx) -> Allowed {
        let t = ctx.table;
        let kinds = t.num_kinds();
        let mut mask = vec![0u32; 4 * kinds];
        for side in 0..2 {
            for (wi, way) in [Way::Forward, Way::Converse].into_iter().enumerate() {
                for k in 0..kinds {
                    let mut m = 0u32;
                    for rel in RelationType::ALL {
                        let Some(spec) = t.step(*rel, way) else { continue };
                        let c = if side == 0 { t.compose(k as Kind, spec.kind) } else { t.compose(spec.kind, k as Kind) };
                        if c != NONE && t.strength(c) >= ctx.min_strength {
                            debug_assert!(rel.code() < 32);
                            m |= 1 << rel.code();
                        }
                    }
                    mask[(side * 2 + wi) * kinds + k] = m;
                }
            }
        }
        Allowed { kinds, mask }
    }

    #[inline]
    fn get(&self, side: usize, way: Way, kind: Kind) -> u32 {
        self.mask[(side * 2 + way as usize) * self.kinds + kind as usize]
    }
}

/// Edge lists longer than this are cut into the runs of allowed relations by
/// binary search; shorter ones are scanned with a per-edge mask test.
const RUN_SEARCH_MIN: usize = 48;

/// One search run.  `penalty` adds cost to claims (penalty method for k routes).
pub fn search(ctx: &QueryCtx, penalty: &AHashMap<ClaimIx, f64>) -> (Option<RawPath>, Stats) {
    search_capped(ctx, penalty, f64::INFINITY, ctx.max_expansions)
}

/// `search` that returns only a route of (penalized) cost <= `cap` and stops as
/// soon as no such route can remain: alternative routes are bounded by stretch.
/// `budget` replaces `ctx.max_expansions` for this run (alternatives get less).
pub fn search_capped(
    ctx: &QueryCtx,
    penalty: &AHashMap<ClaimIx, f64>,
    cap: f64,
    budget: usize,
) -> (Option<RawPath>, Stats) {
    let mut st = Stats::default();
    let n = ctx.snap.num_nodes() as usize;
    let allowed = Allowed::new(ctx);
    let mut sides = [Side::new(true, ctx.s, n), Side::new(false, ctx.t, n)];
    let (ps, pt) = (ctx.potential(ctx.s), ctx.potential(ctx.t));
    let delta = pt - ps;
    sides[0].heap.push(Item { key: 0.0, idx: 0 });
    sides[1].heap.push(Item { key: 0.0, idx: 0 });
    let mut best: Option<Best> = None;
    let mut scratch = Vec::new();

    loop {
        let (tf, tb) = (sides[0].top(), sides[1].top());
        let (Some(tf), Some(tb)) = (tf, tb) else { break };
        if let Some(b) = &best {
            if tf + tb >= b.total + delta - 1e-9 {
                break;
            }
        }
        if tf + tb > cap + delta + 1e-9 {
            st.capped = true;
            break;
        }
        let which = if !ctx.bidirectional || tf <= tb { 0 } else { 1 };
        let item = sides[which].heap.pop().expect("top() saw it");
        let mut lab = sides[which].labels[item.idx as usize];
        if !lab.alive {
            continue;
        }
        if lab.pending {
            match decide(ctx, &mut sides[which], item, &mut st) {
                Decided::Expand(l) => {
                    lab = l;
                    meet(ctx, &sides, which, item.idx, &mut best, &mut scratch);
                }
                Decided::Requeued(new) => {
                    meet(ctx, &sides, which, new, &mut best, &mut scratch);
                    continue;
                }
                Decided::Dropped => continue,
            }
        }
        st.expanded += 1;
        if st.expanded > budget {
            st.truncated = true;
            break;
        }
        // the best route so far (or the cap) minus the other side's smallest key
        let bound = best.as_ref().map_or(cap, |b| b.total.min(cap)) + delta + 1e-9;
        let prune_at = bound - if which == 0 { tb } else { tf };
        expand(ctx, &allowed, penalty, &mut sides, which, item.idx, lab, &mut best, &mut st, &mut scratch, ps, pt, prune_at);
    }
    st.labels = sides[0].labels.len() + sides[1].labels.len();
    st.derive_scanned = ctx.derivable_if_computed().map_or(0, |d| d.scanned);
    let path = best.filter(|b| b.total <= cap + 1e-9).map(|b| {
        let mut steps = sides[0].steps(b.f);
        steps.extend(sides[1].steps(b.b));
        RawPath { steps, kind: b.kind }
    });
    st.nodes = sides[0].touched.len() + sides[1].touched.len();
    let [f, b] = sides;
    f.recycle();
    b.recycle();
    (path, st)
}

enum Decided {
    /// Premises satisfied: same cost, expand now.
    Expand(Label),
    /// Some premises missing: re-inserted at its true, higher cost.
    Requeued(u32),
    /// Rejected (strict mode) or subsumed.
    Dropped,
}

/// Decide the premises of a popped pending label (lazy edge evaluation: most
/// conjunctive labels never reach the top of the heap, so most premises are never
/// decided).  Sound because a pending cost is a lower bound of the decided one.
fn decide(ctx: &QueryCtx, side: &mut Side, item: Item, st: &mut Stats) -> Decided {
    let mut lab = side.labels[item.idx as usize];
    let parent = side.labels[lab.parent as usize].node;
    let (from, to) = if side.forward { (parent, lab.node) } else { (lab.node, parent) };
    let missing = ctx.missing_premises(lab.claim, from).count() as u16;
    side.labels[item.idx as usize].alive = false;
    if missing > 0 && ctx.strict {
        st.conj_rejected += 1;
        return Decided::Dropped;
    }
    lab.pending = false;
    if missing == 0 {
        if side.settle_dominance(&lab, item.idx, ctx.table, ctx.min_strength) {
            return Decided::Dropped;
        }
        side.labels[item.idx as usize] = lab;
        return Decided::Expand(lab);
    }
    let mut step = RawStep {
        from,
        to,
        claim: lab.claim,
        rel: RelationType::from_code(lab.rel).expect("stored from a valid relation"),
        way: if lab.way == 0 { Way::Forward } else { Way::Converse },
        state: EvidenceState::from_code(lab.state).expect("stored from a valid state"),
        missing: 0,
    };
    let before = ctx.step_cost(&step);
    step.missing = missing;
    let extra = ctx.step_cost(&step) - before;
    lab.g += extra;
    lab.missing = missing;
    match side.insert(lab, item.key + extra, ctx.table, ctx.min_strength) {
        Some(new) => Decided::Requeued(new),
        None => Decided::Dropped,
    }
}

#[allow(clippy::too_many_arguments)]
fn expand(
    ctx: &QueryCtx,
    allowed: &Allowed,
    penalty: &AHashMap<ClaimIx, f64>,
    sides: &mut [Side; 2],
    which: usize,
    idx: u32,
    lab: Label,
    best: &mut Option<Best>,
    st: &mut Stats,
    scratch: &mut Vec<NodeIx>,
    ps: f64,
    pt: f64,
    prune_at: f64,
) {
    let forward = which == 0;
    let x = lab.node;
    if lab.hops as u32 >= ctx.max_hops as u32 {
        return;
    }
    let semantic = ctx.min_strength >= 1;
    // premises are decided at relaxation only when that is a bit lookup (exact closure)
    let eager = ctx.derivable_if_computed().is_some_and(|d| d.is_exact());
    // forward side at u: out-edges walk forward, in-edges walk against the stored
    // direction.  Backward side at v: in-edges are forward steps u -> v, out-edges
    // are converse steps u -> v.
    let lists = if forward {
        [(ctx.out.edges(x), Way::Forward), (ctx.inn.edges(x), Way::Converse)]
    } else {
        [(ctx.inn.edges(x), Way::Forward), (ctx.out.edges(x), Way::Converse)]
    };
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (edges, way) in lists {
        let mask = allowed.get(which, way, lab.kind);
        if mask == 0 || edges.is_empty() {
            continue;
        }
        runs.clear();
        if edges.len() < RUN_SEARCH_MIN {
            runs.push((0, edges.len()));
        } else {
            // edges are sorted by relation within a node: cut out the allowed runs
            let mut m = mask;
            let mut lo = 0usize;
            while m != 0 && lo < edges.len() {
                let code = m.trailing_zeros() as u8;
                m &= m - 1;
                let a = lo + edges.rel[lo..].partition_point(|&r| r < code);
                let b = a + edges.rel[a..].partition_point(|&r| r == code);
                if b > a {
                    runs.push((a, b));
                }
                lo = b;
            }
        }
        for &(a, b) in &runs {
            st.relaxed += b - a;
            for i in a..b {
                let code = edges.rel[i];
                let state = edges.state[i];
                if mask & (1u32 << code) == 0 || ctx.state_mask & (1u32 << state) == 0 {
                    continue;
                }
                let rel = RelationType::from_code(code).expect("verified snapshot");
                let spec = ctx.table.step(rel, way).expect("allowed mask implies a step");
                let kind = if forward { ctx.table.compose(lab.kind, spec.kind) } else { ctx.table.compose(spec.kind, lab.kind) };
                let y = edges.nbr[i];
                let hops = lab.hops + 1;
                let (to_t, from_s) = ctx.index.hop_lb2(y, ctx.t, ctx.s, semantic);
                if hops as u32 + if forward { to_t } else { from_s } > ctx.max_hops as u32 {
                    continue;
                }
                let (from, to) = if forward { (x, y) } else { (y, x) };
                let claim = edges.claim[i];
                let mut step = RawStep {
                    from,
                    to,
                    claim,
                    rel,
                    way,
                    state: EvidenceState::from_code(state).expect("verified snapshot"),
                    missing: 0,
                };
                let mut pending = false;
                if spec.conjunction == Conjunction::RequireAll && ctx.snap.conjunct_count(claim) > 1 {
                    if eager {
                        step.missing = ctx.missing_premises(claim, from).count() as u16;
                        if step.missing > 0 && ctx.strict {
                            st.conj_rejected += 1;
                            continue;
                        }
                    } else {
                        pending = true;
                    }
                }
                let extra = if penalty.is_empty() { 0.0 } else { penalty.get(&claim).copied().unwrap_or(0.0) };
                let g = lab.g + ctx.step_cost(&step) + extra;
                let p = ctx.potential_from(to_t, from_s);
                let key = if forward { g + p - ps } else { g - p + pt } + ctx.kind_bound(kind);
                // keys of the other side only grow: with no label of its own at y yet,
                // no route through this label can beat `prune_at`
                if key > prune_at && sides[1 - which].head(y) == NIL {
                    continue;
                }
                let l = Label {
                    node: y,
                    parent: idx,
                    next_at_node: NIL,
                    claim,
                    g,
                    kind,
                    hops,
                    rel: code,
                    way: way as u8,
                    state,
                    missing: step.missing,
                    alive: true,
                    pending,
                };
                let Some(new) = sides[which].insert(l, key, ctx.table, ctx.min_strength) else { continue };
                if !pending {
                    meet(ctx, sides, which, new, best, scratch);
                }
            }
        }
    }
}

/// Check a decided label against the other side's decided labels at the same node.
fn meet(ctx: &QueryCtx, sides: &[Side; 2], which: usize, new: u32, best: &mut Option<Best>, scratch: &mut Vec<NodeIx>) {
    let other = 1 - which;
    let l = sides[which].labels[new as usize];
    debug_assert!(!l.pending);
    let mut j = sides[other].head(l.node);
    while j != NIL {
        let oi = j;
        let o = sides[other].labels[oi as usize];
        j = o.next_at_node;
        if !o.alive || o.pending || l.hops as u32 + o.hops as u32 > ctx.max_hops as u32 {
            continue;
        }
        let (f, b) = if which == 0 { (new, oi) } else { (oi, new) };
        let (lf, lb) = (sides[0].labels[f as usize], sides[1].labels[b as usize]);
        let kind = ctx.table.compose(lf.kind, lb.kind);
        if kind == NONE || kind == EMPTY || ctx.table.strength(kind) < ctx.min_strength {
            continue;
        }
        let total = lf.g + lb.g + ctx.terminal_cost(kind);
        if best.as_ref().is_some_and(|x| x.total <= total) {
            continue;
        }
        // a route visits each node once
        scratch.clear();
        sides[0].chain_nodes(f, scratch);
        let pre = scratch.len();
        sides[1].chain_nodes(lb.parent, scratch);
        let mut seen = AHashSet::with_capacity(scratch.len());
        if !scratch.iter().all(|n| seen.insert(*n)) || pre == 0 {
            continue;
        }
        *best = Some(Best { total, f, b, kind });
    }
}
