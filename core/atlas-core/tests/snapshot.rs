//! Compile the committed fixture registry and check the snapshot against it.
//! Fixture: tests/fixtures/mini_atlas.db (built by make_fixture.py through the Python Store).

use atlas_core::{
    CompileOptions, Dir, EntityType, Error, EvidenceState, RelationType, Role, Snapshot, compile,
};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::PathBuf;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini_atlas.db")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("atlas-core-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn build(name: &str) -> (Snapshot, atlas_core::CompileReport) {
    let mut opts = CompileOptions::new(fixture(), scratch(name));
    opts.threads = 4;
    let report = compile(&opts).expect("compile fixture");
    (Snapshot::open(&opts.out_dir).expect("open via CURRENT"), report)
}

fn node(s: &Snapshot, id: &str) -> u32 {
    s.resolve(id).unwrap_or_else(|| panic!("{id} missing"))
}

#[test]
fn counts_and_integrity() {
    let (s, r) = build("counts");
    let max_seq: i64 =
        Connection::open(fixture()).unwrap().query_row("SELECT MAX(seq) FROM events", [], |r| r.get(0)).unwrap();
    assert_eq!((r.nodes, r.claims, r.edges, r.groups, r.sources), (19, 18, 18, 1, 1));
    assert_eq!(s.header().source_event_seq, max_seq as u64);
    assert_eq!(s.content_hash_hex(), r.content_hash);
    s.verify().expect("fresh snapshot verifies");
    let st = s.stats();
    assert_eq!(st.merged_nodes, 1);
    assert_eq!(st.unknown_by_type.get("concept"), Some(&1), "c:lonely has no claims: unknown, not isolated");
    assert_eq!(st.claims_by_state.get("superseded"), Some(&1));
}

#[test]
fn compile_is_deterministic() {
    let (_, a) = build("det-a");
    let (_, b) = build("det-b");
    assert_eq!(a.content_hash, b.content_hash);
}

#[test]
fn labels_follow_entity_label_fallback() {
    let (s, _) = build("labels");
    let p = node(&s, "s:pythagoras");
    assert_eq!(s.label(p, "es"), "Teorema de Pitágoras");
    assert_eq!(s.label(p, "en"), "Pythagorean theorem");
    let dup = s.find_node("s:pythagoras-dup").unwrap();
    assert_eq!(s.label(dup, "es"), "Pythagoras theorem (duplicate)", "es falls back to en");
    let lonely = node(&s, "c:lonely");
    assert_eq!(s.label(lonely, "en"), "Concepto sin afirmaciones", "en falls back to the first label");
    assert_eq!(s.node_type(lonely), EntityType::Concept);
    assert!(s.evidence(lonely).is_unknown());
    assert_eq!(s.find_node("nope"), None);
}

#[test]
fn merges_resolve_like_the_store() {
    let (s, _) = build("merges");
    let p = node(&s, "s:pythagoras");
    let dup = s.find_node("s:pythagoras-dup").unwrap();
    assert!(s.is_merged(dup));
    assert_eq!(s.canonical(dup), p);
    assert_eq!(s.merged_from(p), vec![dup]);
    assert_eq!(s.out_edges(dup).len() + s.in_edges(dup).len(), 0, "edges live on the canonical node");
    let rt = node(&s, "c:right-triangle");
    let prereq: Vec<&str> = s
        .csr(Dir::Out)
        .edges_rel(rt, RelationType::PrerequisiteFor)
        .iter()
        .map(|e| {
            assert_eq!(e.nbr, p);
            s.claim_id(e.claim)
        })
        .collect();
    assert_eq!(prereq.len(), 2);
    assert!(prereq.contains(&"c:dup-prereq") && prereq.contains(&"c:rt-prereq"));
}

#[test]
fn conjunctions_alternatives_and_provenance() {
    let (s, _) = build("andor");
    let c = s.find_claim("c:pyth-euclid").unwrap();
    assert_eq!(s.claim_relation(c), RelationType::DependsOnLogically);
    let premises: Vec<&str> = s.claim_targets(c).iter().map(|&n| s.node_id(n)).collect();
    assert_eq!(premises, ["s:sas-congruence", "s:parallelogram-area"]);
    assert_eq!(s.conjunct_count(c), 2);
    let ctx: Vec<(Role, &str)> = s.claim_context(c).map(|(r, n)| (r, s.node_id(n))).collect();
    assert_eq!(
        ctx,
        [(Role::Via, "p:euclid-i47"), (Role::Context, "a:euclidean-geometry"), (Role::Assumption, "s:parallel-postulate")]
    );
    let g = s.claim_group(c).unwrap();
    assert_eq!(s.find_group("g:pythagoras-proofs"), Some(g));
    let alts: Vec<&str> = s.group_claims(g).iter().map(|&c| s.claim_id(c)).collect();
    assert_eq!(alts, ["c:pyth-euclid", "c:pyth-similarity"]);
    let prov: Vec<_> = s.claim_provenance(c).collect();
    assert_eq!(prov.len(), 1);
    assert_eq!((prov[0].source_id, prov[0].locator, prov[0].license), ("src:fixture-hand", "Elements I.47", "CC0-1.0"));
    assert_eq!(s.source_title(prov[0].source), "Hand-written test fixture (MPM)");
    let p = node(&s, "s:pythagoras");
    let deps = s.csr(Dir::Out).edges_rel(p, RelationType::DependsOnLogically);
    assert_eq!(deps.len(), 3, "two premises of one claim + one of the alternative");
    assert_eq!(s.area(p), Some(node(&s, "a:euclidean-geometry")));
}

#[test]
fn superseded_claims_leave_the_graph_but_keep_their_record() {
    let (s, _) = build("versions");
    let old = s.find_claim("c:cosines-gen").unwrap();
    let new = s.find_claim("c:cosines-gen-v2").unwrap();
    assert_eq!(s.claim_state(old), EvidenceState::Superseded);
    let law = node(&s, "s:law-of-cosines");
    let gens: Vec<u32> = s.out_edges(law).iter().map(|e| e.claim).collect();
    assert_eq!(gens, [new]);
    assert_eq!(s.claim_assumptions(new).collect::<Vec<_>>(), ["Euclidean plane"]);
    assert_eq!(s.claim_validity_domain(new), Some("real inner product spaces"));
    assert_eq!(s.claim_assumptions(old).len(), 0);
}

/// Independent SQL reading of `Store.iter_edges` semantics (current states, merges
/// resolved, one edge per object/premise) must equal the CSR, in both directions.
#[test]
fn csr_equals_store_edge_semantics() {
    let (s, _) = build("edges");
    let db = Connection::open(fixture()).unwrap();
    let red: HashMap<String, String> = db
        .prepare("SELECT id, merged_into FROM entities WHERE merged_into IS NOT NULL")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let follow = |mut id: String| {
        while let Some(n) = red.get(&id) {
            id = n.clone();
        }
        id
    };
    let mut want: Vec<(String, String, String, String, String)> = db
        .prepare(
            "SELECT c.id, c.relation, c.evidence_state, s.entity_id, t.entity_id FROM claims c \
             JOIN claim_participants s ON s.claim_id = c.id AND s.role = 'subject' \
             JOIN claim_participants t ON t.claim_id = c.id AND t.role IN ('object','premise') \
             WHERE c.evidence_state NOT IN ('superseded','retracted')",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .map(|(c, rel, st, a, b)| (c, rel, st, follow(a), follow(b)))
        .collect();
    want.sort();
    for dir in [Dir::Out, Dir::In] {
        let csr = s.csr(dir);
        let mut got = Vec::new();
        for n in 0..s.num_nodes() {
            for e in csr.edges(n).iter() {
                let (a, b) = if dir == Dir::Out { (n, e.nbr) } else { (e.nbr, n) };
                got.push((
                    s.claim_id(e.claim).to_owned(),
                    e.relation.as_str().to_owned(),
                    e.state.as_str().to_owned(),
                    s.node_id(a).to_owned(),
                    s.node_id(b).to_owned(),
                ));
            }
        }
        got.sort();
        assert_eq!(got, want, "{dir:?}");
    }
}

#[test]
fn tampering_is_detected() {
    let (s, _) = build("tamper");
    let bytes = std::fs::read(s.path()).unwrap();
    let dir = scratch("tamper-copy");

    let mut flipped = bytes.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 0x01; // inside the last section (InState)
    let f = dir.join("flipped.snap");
    std::fs::write(&f, &flipped).unwrap();
    let t = Snapshot::open(&f).expect("structure is intact");
    assert!(matches!(t.verify(), Err(Error::Integrity(_))));

    let mut bad_magic = bytes.clone();
    bad_magic[0] = b'X';
    let f = dir.join("magic.snap");
    std::fs::write(&f, &bad_magic).unwrap();
    assert!(matches!(Snapshot::open(&f), Err(Error::Format(_))));

    let f = dir.join("short.snap");
    std::fs::write(&f, &bytes[..bytes.len() / 2]).unwrap();
    assert!(matches!(Snapshot::open(&f), Err(Error::Format(_))));
}

#[test]
fn missing_registry_is_not_found() {
    let opts = CompileOptions::new(scratch("missing").join("none.db"), scratch("missing-out"));
    assert!(matches!(compile(&opts), Err(Error::NotFound(_))));
}
