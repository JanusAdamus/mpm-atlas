//! What can be established from the start of a route (R7).
//!
//! A `depends_on_logically` claim is an AND-node: it establishes its subject once
//! all its premises are established; alternative claims of one subject are
//! OR-branches.  Equivalences carry establishment both ways.  Two strategies:
//!
//! 1. Forward chaining from the start and the known nodes (Dowling & Gallier 1984,
//!    linear-time Horn satisfiability), bounded by `budget` edge scans.  When it
//!    finishes, membership is exact.
//! 2. When the budget runs out (dense registries: on the 1M synthetic graph the
//!    closure of almost any node is 86% of the graph and costs ~0.6 s), membership
//!    of a premise is decided by goal-directed backward chaining over the AND/OR
//!    graph, depth- and visit-bounded, seeded with the partial forward set.
//!    Backward searches share what they settle for good: nodes *proven* (a
//!    derivation exists) and nodes *refuted* (every AND/OR branch failed without
//!    touching a bound or a cycle cut, so no derivation exists).  Both facts are
//!    query-order independent; only a bounded failure may depend on what earlier
//!    searches proved, and that answer is always sound.
//!
//! Both are sound (never call a premise established without a derivation); the
//! bounded parts may miss a derivation, and the engine then says so.
//!
//! Buffers are dense per node and pooled per thread (reset by touched lists), so a
//! query pays for the edges it scans, not for the size of the graph.

use ahash::{AHashMap, AHashSet};
use atlas_core::{ClaimIx, Dir, NodeIx, RelationType, Snapshot};
use std::cell::{Cell, RefCell};
use std::time::Instant;

/// Depth of a backward derivation (AND/OR levels).
pub const PROVE_DEPTH: u8 = 8;
/// Nodes one backward search may visit before giving up.
pub const PROVE_VISITS: usize = 4096;

/// How a node came to be established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Start,
    Known,
    /// Fired claim (a dependency whose premises were all established, or an equivalence).
    Claim(ClaimIx),
}

// dense encoding of `why`: 0 = not established
const W_START: u32 = 1;
const W_KNOWN: u32 = 2;
const W_CLAIM: u32 = 3;

fn decode(w: u32) -> Option<Why> {
    match w {
        0 => None,
        W_START => Some(Why::Start),
        W_KNOWN => Some(Why::Known),
        c => Some(Why::Claim(c - W_CLAIM)),
    }
}

/// Per-thread reusable buffers (all-zero `why` between uses).
#[derive(Default)]
struct Bufs {
    why: Vec<u32>,
    touched: Vec<NodeIx>,
    fired: AHashMap<ClaimIx, u16>,
    queue: Vec<NodeIx>,
}

thread_local! {
    static POOL: RefCell<Vec<Bufs>> = const { RefCell::new(Vec::new()) };
}

fn take_bufs(n: usize) -> Bufs {
    let mut b = POOL.with(|p| p.borrow_mut().pop()).unwrap_or_default();
    if b.why.len() != n {
        b.why = vec![0; n];
        b.touched.clear();
    }
    b
}

pub struct Derivable<'a> {
    snap: &'a Snapshot,
    state_mask: u32,
    bufs: Bufs,
    /// Nodes in the forward closure.
    pub size: usize,
    /// False when the forward budget ran out before the closure was complete.
    pub complete: bool,
    pub scanned: usize,
    /// Answer per premise asked (backward search), and the shared settled facts.
    memo: RefCell<AHashMap<NodeIx, bool>>,
    proven: RefCell<AHashMap<NodeIx, Why>>,
    refuted: RefCell<AHashSet<NodeIx>>,
    /// Backward searches run, and whether any hit its depth or visit bound.
    pub backward_checks: Cell<usize>,
    pub backward_bounded: Cell<bool>,
    /// Time spent in the forward closure and the backward searches.
    pub nanos: Cell<u64>,
}

impl Drop for Derivable<'_> {
    fn drop(&mut self) {
        let mut b = std::mem::take(&mut self.bufs);
        for &n in &b.touched {
            b.why[n as usize] = 0;
        }
        b.touched.clear();
        b.fired.clear();
        b.queue.clear();
        POOL.with(|p| {
            let mut p = p.borrow_mut();
            if p.len() < 2 {
                p.push(b);
            }
        });
    }
}

impl Derivable<'_> {
    /// In the forward closure (no backward search).
    #[inline]
    pub fn in_forward(&self, n: NodeIx) -> bool {
        self.bufs.why[n as usize] != 0
    }

    /// Is `n` established from the start (and the known nodes)?
    pub fn contains(&self, n: NodeIx) -> bool {
        if self.in_forward(n) {
            return true;
        }
        if self.complete {
            return false;
        }
        if let Some(&r) = self.memo.borrow().get(&n) {
            return r;
        }
        let t0 = Instant::now();
        let mut p = Prover { d: self, failed: AHashMap::new(), stack: AHashSet::new(), visits: 0, bounded: false };
        let (ok, _) = p.go(n, PROVE_DEPTH);
        self.backward_checks.set(self.backward_checks.get() + 1);
        if p.bounded && !ok {
            self.backward_bounded.set(true);
        }
        self.memo.borrow_mut().insert(n, ok);
        self.nanos.set(self.nanos.get() + t0.elapsed().as_nanos() as u64);
        ok
    }

    /// Exact membership (forward closure finished) or proven-only (bounded search).
    pub fn is_exact(&self) -> bool {
        self.complete
    }

    pub fn why(&self, n: NodeIx) -> Option<Why> {
        decode(self.bufs.why[n as usize])
            .or_else(|| if self.contains(n) { self.proven.borrow().get(&n).copied() } else { None })
    }

    /// Claims of the derivation of `n`, breadth first, at most `cap`.
    pub fn derivation(&self, n: NodeIx, cap: usize) -> (Vec<ClaimIx>, bool) {
        let snap = self.snap;
        let mut out = Vec::new();
        let mut queue = std::collections::VecDeque::from([n]);
        let mut seen = AHashSet::new();
        while let Some(x) = queue.pop_front() {
            if !seen.insert(x) {
                continue;
            }
            if let Some(Why::Claim(c)) = self.why(x) {
                if out.len() == cap {
                    return (out, true);
                }
                out.push(c);
                if snap.claim_relation(c) == RelationType::DependsOnLogically {
                    queue.extend(snap.claim_targets(c).iter().copied());
                } else {
                    let s = snap.claim_subject(c);
                    queue.extend(snap.claim_targets(c).iter().copied().chain([s]).filter(|&y| y != x));
                }
            }
        }
        (out, false)
    }
}

/// One backward AND/OR search.  Its local failures are depth-dependent; settled
/// facts go to the shared `proven` / `refuted` sets.
struct Prover<'d, 'a> {
    d: &'d Derivable<'a>,
    /// Deepest remaining depth at which a node failed (this search only).
    failed: AHashMap<NodeIx, u8>,
    stack: AHashSet<NodeIx>,
    visits: usize,
    bounded: bool,
}

impl Prover<'_, '_> {
    /// (established, settled): `settled` is false when the answer leaned on a
    /// bound or a cycle cut and may change in another search.
    fn go(&mut self, n: NodeIx, depth: u8) -> (bool, bool) {
        if self.d.in_forward(n) || self.d.proven.borrow().contains_key(&n) {
            return (true, true);
        }
        if self.d.refuted.borrow().contains(&n) {
            return (false, true);
        }
        if depth == 0 {
            self.bounded = true;
            return (false, false);
        }
        if self.failed.get(&n).is_some_and(|&f| f >= depth) || self.stack.contains(&n) {
            return (false, false);
        }
        self.visits += 1;
        if self.visits > PROVE_VISITS {
            self.bounded = true;
            return (false, false);
        }
        let snap = self.d.snap;
        let ok = |s: atlas_core::EvidenceState| self.d.state_mask & s.bit() != 0;
        self.stack.insert(n);
        let mut found = None;
        let mut settled = true;
        // AND-nodes: each dependency claim of n, visited once (at the edge to its first target)
        let deps = snap.csr(Dir::Out).edges_rel(n, RelationType::DependsOnLogically);
        for e in deps.iter() {
            let targets = snap.claim_targets(e.claim);
            if !ok(e.state) || targets[0] != e.nbr {
                continue;
            }
            let mut all = true;
            for &t in targets {
                let (o, s) = self.go(t, depth - 1);
                if !o {
                    settled &= s;
                    all = false;
                    break;
                }
            }
            if all {
                found = Some(e.claim);
                break;
            }
        }
        if found.is_none() {
            let eq = [Dir::Out, Dir::In].map(|d| snap.csr(d).edges_rel(n, RelationType::EquivalentTo));
            'eq: for list in eq {
                for e in list.iter() {
                    if !ok(e.state) {
                        continue;
                    }
                    let (o, s) = self.go(e.nbr, depth - 1);
                    if o {
                        found = Some(e.claim);
                        break 'eq;
                    }
                    settled &= s;
                }
            }
        }
        self.stack.remove(&n);
        match found {
            Some(c) => {
                self.d.proven.borrow_mut().insert(n, Why::Claim(c));
                (true, true)
            }
            None => {
                if settled {
                    self.d.refuted.borrow_mut().insert(n);
                }
                let f = self.failed.entry(n).or_insert(0);
                *f = (*f).max(depth);
                (false, settled)
            }
        }
    }
}

/// Forward chaining from `start` and `known` (Horn closure, linear in the edges
/// scanned), stopping after `budget` edge scans.
pub fn closure<'a>(snap: &'a Snapshot, start: NodeIx, known: &[NodeIx], state_mask: u32, budget: usize) -> Derivable<'a> {
    let t0 = Instant::now();
    let mut b = take_bufs(snap.num_nodes() as usize);
    let Bufs { why, touched, fired, queue } = &mut b;
    let mut establish = |n: NodeIx, w: u32, queue: &mut Vec<NodeIx>| {
        if why[n as usize] == 0 {
            why[n as usize] = w;
            touched.push(n);
            queue.push(n);
        }
    };
    establish(start, W_START, queue);
    for &k in known {
        establish(k, W_KNOWN, queue);
    }
    let (out, inn) = (snap.csr(Dir::Out), snap.csr(Dir::In));
    let mut scanned = 0usize;
    let mut complete = true;
    let ok = |s: u8| state_mask & (1u32 << s) != 0;
    while let Some(n) = queue.pop() {
        // claims where n is a premise: subject --depends_on--> n
        let deps = inn.edges_rel(n, RelationType::DependsOnLogically);
        let eq_in = inn.edges_rel(n, RelationType::EquivalentTo);
        let eq_out = out.edges_rel(n, RelationType::EquivalentTo);
        scanned += deps.len() + eq_in.len() + eq_out.len();
        for i in 0..deps.len() {
            if !ok(deps.state[i]) {
                continue;
            }
            let c = deps.claim[i];
            let need = snap.conjunct_count(c);
            if need > 1 {
                let f = fired.entry(c).or_insert(0);
                *f = f.saturating_add(1);
                if (*f as usize) < need {
                    continue;
                }
            }
            establish(deps.nbr[i], c + W_CLAIM, queue);
        }
        for e in [eq_in, eq_out] {
            for i in 0..e.len() {
                if ok(e.state[i]) {
                    establish(e.nbr[i], e.claim[i] + W_CLAIM, queue);
                }
            }
        }
        if scanned >= budget && !queue.is_empty() {
            complete = false;
            break;
        }
    }
    let size = b.touched.len();
    Derivable {
        snap,
        state_mask,
        bufs: b,
        size,
        complete,
        scanned,
        memo: RefCell::new(AHashMap::new()),
        proven: RefCell::new(AHashMap::new()),
        refuted: RefCell::new(AHashSet::new()),
        backward_checks: Cell::new(0),
        backward_bounded: Cell::new(false),
        nanos: Cell::new(t0.elapsed().as_nanos() as u64),
    }
}
