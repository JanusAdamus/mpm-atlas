//! The search against exhaustive enumeration on a synthetic registry
//! (atlas_core::synth into a copy of tests/fixtures/empty_v1.db): for every
//! objective and every search mode (bidirectional or not, ALT or not), the first
//! route's cost equals the minimum over all simple, walkable, authorized routes
//! within the hop limit, and the route itself is one of them.

use ahash::AHashMap;
use atlas_core::synth::{SynthConfig, generate};
use atlas_core::{CompileOptions, Dir, EvidenceState, NodeIx, Snapshot, compile};
use atlas_paths::algebra::{EMPTY, Kind, NONE, Way};
use atlas_paths::search::{QueryCtx, RawStep, Weights, search};
use atlas_paths::{CompositionTable, GraphIndex, Objective};
use std::path::Path;

const MAX_HOPS: u8 = 4;

fn synthetic() -> Snapshot {
    let dir = std::env::temp_dir().join(format!("atlas-paths-opt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("synth.db");
    std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/empty_v1.db"), &db).unwrap();
    generate(&db, &SynthConfig { entities: 1500, edges: 4500, seed: 7 }).expect("synth");
    let opts = CompileOptions::new(&db, dir.join("snap"));
    compile(&opts).expect("compile");
    Snapshot::open(dir.join("snap")).unwrap()
}

struct Brute<'a, 'c> {
    ctx: &'c QueryCtx<'a>,
    best: AHashMap<NodeIx, f64>,
    on_path: Vec<NodeIx>,
}

impl Brute<'_, '_> {
    /// Every simple walkable route from s: record the cheapest authorized cost per end node.
    fn dfs(&mut self, x: NodeIx, kind: Kind, g: f64, hops: u8) {
        if hops == MAX_HOPS {
            return;
        }
        let (snap, table, ctx) = (self.ctx.snap, self.ctx.table, self.ctx);
        for (dir, way) in [(Dir::Out, Way::Forward), (Dir::In, Way::Converse)] {
            for e in snap.csr(dir).edges(x).iter() {
                if ctx.state_mask & e.state.bit() == 0 || self.on_path.contains(&e.nbr) {
                    continue;
                }
                let Some(spec) = table.step(e.relation, way) else { continue };
                let k = table.compose(kind, spec.kind);
                if k == NONE || table.strength(k) < ctx.min_strength {
                    continue;
                }
                let missing = if spec.conjunction == atlas_paths::algebra::Conjunction::RequireAll
                    && snap.conjunct_count(e.claim) > 1
                {
                    ctx.missing_premises(e.claim, x).count() as u16
                } else {
                    0
                };
                if missing > 0 && ctx.strict {
                    continue;
                }
                let step = RawStep { from: x, to: e.nbr, claim: e.claim, rel: e.relation, way, state: e.state, missing };
                let g2 = g + ctx.step_terms(&step).weighted(&ctx.w);
                if e.nbr == ctx.t || self.ctx.t == NodeIx::MAX {
                    let total = g2 + ctx.terminal_cost(k);
                    let b = self.best.entry(e.nbr).or_insert(f64::INFINITY);
                    *b = b.min(total);
                }
                self.on_path.push(e.nbr);
                self.dfs(e.nbr, k, g2, hops + 1);
                self.on_path.pop();
            }
        }
    }
}

fn route_cost(ctx: &QueryCtx, steps: &[RawStep]) -> f64 {
    let mut k = EMPTY;
    let mut g = 0.0;
    let mut nodes = vec![ctx.s];
    for s in steps {
        let spec = ctx.table.step(s.rel, s.way).expect("walkable step");
        k = ctx.table.compose(k, spec.kind);
        assert!(k != NONE, "route stays authorized");
        g += ctx.step_terms(s).weighted(&ctx.w);
        assert_eq!(*nodes.last().unwrap(), s.from, "steps are contiguous");
        assert!(!nodes.contains(&s.to), "route is simple");
        nodes.push(s.to);
    }
    assert_eq!(*nodes.last().unwrap(), ctx.t);
    assert!(steps.len() <= MAX_HOPS as usize);
    g + ctx.terminal_cost(k)
}

#[allow(clippy::too_many_arguments)]
fn ctx_for<'a>(
    snap: &'a Snapshot,
    table: &'a CompositionTable,
    index: &'a GraphIndex,
    s: NodeIx,
    t: NodeIx,
    o: Objective,
    strict: bool,
    derive_budget: usize,
) -> QueryCtx<'a> {
    let w = Weights::from_map(table.objective_weights(o));
    let mask = atlas_core::CURRENT_STATES_MASK & !EvidenceState::Disputed.bit();
    let mut c = QueryCtx::new(snap, table, index, s, t, Vec::new(), w, mask);
    c.max_hops = MAX_HOPS;
    c.strict = strict;
    c.derive_budget = derive_budget;
    c
}

#[test]
fn search_matches_exhaustive_enumeration() {
    let snap = synthetic();
    let table = CompositionTable::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../shapes/composition.json")).unwrap();
    let index = GraphIndex::build_typed(&snap, 8, &table);
    let n = snap.num_nodes();
    let mut checked = 0usize;
    let mut rng = 0x1234_5678u64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut sources = 0;
    while sources < 25 {
        let s = (next() % n as u64) as NodeIx;
        if snap.evidence(s).is_unknown() || snap.out_edges(s).len() + snap.in_edges(s).len() == 0 {
            continue;
        }
        sources += 1;
        for o in Objective::ALL {
            // budget 0: no forward closure, so every conjunctive label is decided lazily
            // by backward chaining when popped
            for (strict, budget) in [(false, 200_000), (true, 200_000), (false, 0), (true, 0)] {
                if budget == 0 && sources > 8 {
                    continue; // backward chaining is slow unoptimized; 8 sources cover it
                }
                // all reachable ends from s in one enumeration (t = MAX means "any")
                let any = ctx_for(&snap, &table, &index, s, NodeIx::MAX, o, strict, budget);
                let mut b = Brute { ctx: &any, best: AHashMap::new(), on_path: vec![s] };
                b.dfs(s, EMPTY, 0.0, 0);
                let mut ends: Vec<(NodeIx, f64)> = b.best.into_iter().collect();
                ends.sort_by_key(|e| e.0);
                for &(t, want) in ends.iter().step_by(ends.len().div_ceil(6).max(1)) {
                    for (bidirectional, heuristic) in [(true, true), (true, false), (false, true), (false, false)] {
                        let mut ctx = ctx_for(&snap, &table, &index, s, t, o, strict, budget);
                        ctx.bidirectional = bidirectional;
                        ctx.heuristic = heuristic;
                        // per-t enumeration: `fresh` depends on t, so recompute the optimum exactly
                        let mut bt = Brute { ctx: &ctx, best: AHashMap::new(), on_path: vec![s] };
                        bt.dfs(s, EMPTY, 0.0, 0);
                        let want_t = bt.best.get(&t).copied().unwrap_or(want);
                        let (p, st) = search(&ctx, &AHashMap::new());
                        assert!(!st.truncated);
                        let p = p.unwrap_or_else(|| panic!("{o:?} strict={strict} {s}->{t}: search found nothing, brute {want_t}"));
                        let got = route_cost(&ctx, &p.steps);
                        assert!(
                            (got - want_t).abs() < 1e-9,
                            "{o:?} strict={strict} budget={budget} bi={bidirectional} alt={heuristic} {s}->{t}: search {got}, brute {want_t}"
                        );
                        checked += 1;
                    }
                }
            }
        }
    }
    eprintln!("{checked} search-vs-exhaustive comparisons");
    assert!(checked > 500, "only {checked} comparisons");
}
