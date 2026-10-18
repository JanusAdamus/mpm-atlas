//! End-to-end over the committed fixture registry (atlas-core/tests/fixtures/mini_atlas.db,
//! built from tests/fixtures/mini_atlas.json through the Python Store): compile a
//! snapshot, build every lane with the deterministic TF-IDF+SVD encoder, search.

use super::*;
use atlas_core::{CompileOptions, compile};
use std::sync::OnceLock;

struct Fixture {
    snap: Snapshot,
    idx: SearchIndex,
    index_root: PathBuf,
}

fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let db = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../atlas-core/tests/fixtures/mini_atlas.db");
        let root = std::env::temp_dir().join(format!("atlas-search-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mut copts = CompileOptions::new(&db, root.join("snapshot"));
        copts.threads = 2;
        compile(&copts).expect("compile fixture");
        let snap = Snapshot::open(root.join("snapshot")).expect("open snapshot");
        let opts = BuildOptions { encoder: EncoderChoice::Lsi, threads: 2, hnsw: true, sessions: 1, intra_threads: 1 };
        let report = SearchIndex::build_with(&snap, &db, &root.join("index"), &opts).expect("build index");
        assert_eq!(report.encoder, format!("lsi-tfidf-svd-{}", report.dim));
        assert!(report.documents >= 17, "{report:?}");
        assert!(report.formula.documents >= 3, "{report:?}");
        let idx = SearchIndex::open(&snap, &root.join("index")).expect("open index");
        Fixture { snap, idx, index_root: root.join("index") }
    })
}

fn search(q: &str) -> (SearchResponse, Explain) {
    let f = fixture();
    let q = SearchQuery { q: q.into(), lang: "es".into(), types: vec![], limit: 10 };
    f.idx.search_explained(&f.snap, &q).expect("search")
}

fn ids(r: &SearchResponse) -> Vec<&str> {
    r.hits.iter().map(|h| h.id.as_str()).collect()
}

#[test]
fn named_object_ranks_the_exact_name_first_with_lane_diagnostics() {
    let (r, e) = search("Pitágoras");
    assert_eq!(e.route.kind, RouteKind::NamedObject);
    assert_eq!(r.hits[0].id, "s:pythagoras", "{:?}", ids(&r));
    let h = &r.hits[0];
    assert_eq!(h.label, "Teorema de Pitágoras");
    assert!(h.lanes.lexical.is_some_and(|l| l.rank <= 2), "{:?}", h.lanes);
    assert!(matches!(h.matched.field.as_str(), "alias" | "label"), "{:?}", h.matched);
    assert!(r.hits.windows(2).all(|w| w[0].score >= w[1].score));
    // English query, English stemming
    let (r, _) = search("pythagorean theorems");
    assert_eq!(r.hits[0].id, "s:pythagoras", "{:?}", ids(&r));
}

#[test]
fn formula_query_finds_the_same_structure_under_renamed_variables() {
    let (r, e) = search("x^2 + y^2 = z^2");
    assert_eq!(e.route.kind, RouteKind::Formula);
    assert_eq!(r.hits[0].id, "s:pythagoras", "{:?}", ids(&r));
    assert_eq!(r.hits[0].lanes.formula.map(|l| l.rank), Some(1));
    assert_eq!(r.hits[0].matched.field, "formula");
    assert!(ids(&r).contains(&"s:law-of-cosines"), "{:?}", ids(&r));
    let (r, _) = search(r"\|v\|^2 = \sum_j |\langle v, u_j\rangle|^2");
    assert_eq!(r.hits[0].id, "s:parseval", "{:?}", ids(&r));
}

#[test]
fn merged_entities_are_found_through_their_survivor() {
    let (r, _) = search("duplicate");
    assert_eq!(r.hits[0].id, "s:pythagoras", "{:?}", ids(&r));
    assert!(!ids(&r).contains(&"s:pythagoras-dup"));
}

#[test]
fn type_filters_from_the_query_and_from_types() {
    let (r, _) = search("pitágoras type:person");
    assert_eq!(r.hits[0].id, "per:pythagoras");
    assert!(r.hits.iter().all(|h| h.entity_type == EntityType::Person), "{:?}", ids(&r));
    let (r, _) = search("teorema -type:statement");
    assert!(r.hits.iter().all(|h| h.entity_type != EntityType::Statement), "{:?}", ids(&r));
    let f = fixture();
    let q = SearchQuery { q: "euclides".into(), lang: "es".into(), types: vec![EntityType::Proof], limit: 5 };
    let r = f.idx.search(&f.snap, &q).unwrap();
    assert!(!r.hits.is_empty() && r.hits.iter().all(|h| h.entity_type == EntityType::Proof), "{:?}", ids(&r));
}

#[test]
fn assumption_filters_use_claim_assumptions() {
    // only c:cosines-gen-v2 (subject s:law-of-cosines) assumes "Euclidean plane"
    let (r, e) = search("cosenos teorema pitágoras assume:euclidean_plane");
    assert_eq!(e.filters.assume, ["euclidean_plane"]);
    assert_eq!(ids(&r), ["s:law-of-cosines"]);
    let (r, _) = search("ley de los cosenos -assume:euclidean");
    assert!(!ids(&r).contains(&"s:law-of-cosines"), "{:?}", ids(&r));
}

#[test]
fn single_lane_mode_and_semantic_lane_with_both_dense_methods() {
    let (r, e) = search("triángulo rectángulo hipotenusa lane:semantic");
    assert!(e.lanes.iter().all(|l| (l.lane == Lane::Semantic) == (l.weight > 0.0)));
    assert!(!r.hits.is_empty());
    assert!(r.hits.iter().all(|h| h.lanes.lexical.is_none() && h.lanes.semantic.is_some()));
    // HNSW and the exact scan agree on this tiny corpus
    let f = fixture();
    let qv = f.idx.encode_query("triángulo rectángulo hipotenusa").unwrap();
    let mut idx = SearchIndex::open(&f.snap, &f.index_root).unwrap();
    let h = idx.dense_search(&qv, 5, None).unwrap();
    idx.set_dense_method(DenseMethod::Brute).unwrap();
    let b = idx.dense_search(&qv, 5, None).unwrap();
    assert_eq!(h.first().map(|x| x.0), b.first().map(|x| x.0));
}

#[test]
fn response_has_the_api_shape_and_empty_queries_are_empty() {
    let (r, _) = search("Euclides");
    let v = serde_json::to_value(&r).unwrap();
    for k in ["query", "lang", "limit", "truncated", "hits"] {
        assert!(v.get(k).is_some(), "{k}");
    }
    let h = &v["hits"][0];
    for k in ["id", "type", "label", "matched", "score", "lanes"] {
        assert!(h.get(k).is_some(), "{k}");
    }
    assert!(h["matched"].get("field").is_some() && h["lanes"].get("lexical").is_some());
    let (r, _) = search("   type:person ");
    assert!(r.hits.is_empty());
}

#[test]
fn index_of_another_snapshot_is_refused() {
    let f = fixture();
    let copy = f.index_root.with_file_name("index-tampered");
    let _ = std::fs::remove_dir_all(&copy);
    std::fs::create_dir_all(&copy).unwrap();
    let src = f.idx.dir().join("meta.json");
    let mut meta: serde_json::Value = serde_json::from_slice(&std::fs::read(src).unwrap()).unwrap();
    meta["snapshot_hash"] = "0".repeat(64).into();
    std::fs::write(copy.join("meta.json"), serde_json::to_vec(&meta).unwrap()).unwrap();
    match SearchIndex::open(&f.snap, &copy) {
        Err(Error::Integrity(m)) => assert!(m.contains("built for snapshot"), "{m}"),
        Err(e) => panic!("unexpected error {e}"),
        Ok(_) => panic!("opened an index of another snapshot"),
    }
}

#[test]
fn queries_of_three_words_or_more_add_an_all_words_clause_on_descriptions_and_statements() {
    // regression: without it docstring-fragment queries lost to one-word label hits
    // (AtlasBench v3 dev, en/text nDCG@10 0.635 -> 0.750 with it)
    let f = fixture();
    let lq = f.idx.lexical.parse("between the same parallels", "en").expect("parse");
    assert!(lq.must_clauses() >= 2 * 3, "one Must per word for description and statement");
    let lq = f.idx.lexical.parse("parallel postulate", "en").expect("parse");
    assert_eq!(lq.must_clauses(), 0);
    let q = SearchQuery { q: "lane:lexical between the same parallels".into(), lang: "en".into(), types: vec![], limit: 5 };
    let (r, _) = f.idx.search_explained(&f.snap, &q).expect("search");
    assert_eq!(r.hits[0].id, "s:parallelogram-area", "{:?}", ids(&r));
}
