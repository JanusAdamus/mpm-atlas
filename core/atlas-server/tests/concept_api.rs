//! `/api/concept`, `/api/concept/edges` and the concept blocks of `/api/entity`, `/api/health`
//! and the tiles, over the mini_atlas fixture plus four `member_of_concept` claims.

use atlas_server::{AppState, ServerConfig, router};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;
use tower::ServiceExt;

/// (claim, declaration, concept, confidence, signal)
const MEMBERS: [(&str, &str, &str, f64, &str); 4] = [
    ("c:m1", "s:pythagoras", "c:right-triangle", 0.9, "curated"),
    ("c:m2", "s:pythagoras", "c:lonely", 0.5, "name_doc"),
    ("c:m3", "s:sas-congruence", "c:lonely", 0.8, "zone"),
    ("c:m4", "s:parallelogram-area", "c:lonely", 0.7, "name_doc"),
];

fn manifest() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).canonicalize().unwrap();
    PathBuf::from(dir.to_string_lossy().trim_start_matches(r"\\?\"))
}

fn app() -> &'static Router {
    static APP: OnceLock<Router> = OnceLock::new();
    APP.get_or_init(|| {
        let tmp = std::env::temp_dir().join(format!("atlas-server-concepts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let db = tmp.join("atlas.db");
        std::fs::copy(manifest().join("../atlas-core/tests/fixtures/mini_atlas.db"), &db).unwrap();
        let c = Connection::open(&db).unwrap();
        for (id, s, o, conf, signal) in MEMBERS {
            let quote = format!(r#"{{\"signal\":\"{signal}\",\"evidence\":\"test\",\"rank\":0}}"#);
            let data = format!(
                r#"{{"id":"{id}","relation":"member_of_concept","participants":[{{"role":"subject","entity":"{s}"}},{{"role":"object","entity":"{o}"}}],"provenance":[{{"source_id":"src:fixture-hand","locator":"{s} -> {o}","extractor":"concept-projection","extractor_version":"5","license":"CC0-1.0","retrieved_at":"t","quote":"{quote}"}}],"evidence_state":"model_proposed","confidence":{{"value":{conf},"estimates":"e","calibration":"not calibrated"}}}}"#
            );
            c.execute(
                "INSERT INTO claims VALUES (?1,'member_of_concept','model_proposed',NULL,NULL,NULL,NULL,?2,'t')",
                params![id, data],
            )
            .unwrap();
            c.execute("INSERT INTO claim_participants VALUES (?1,'subject',?2,0)", params![id, s]).unwrap();
            c.execute("INSERT INTO claim_participants VALUES (?1,'object',?2,1)", params![id, o]).unwrap();
            c.execute(
                "INSERT INTO provenance VALUES (?1,NULL,'src:fixture-hand','test','concept-projection','5','CC0-1.0','t',NULL)",
                params![id],
            )
            .unwrap();
        }
        c.execute_batch("PRAGMA user_version = 2").unwrap();
        drop(c);
        let snap = tmp.join("snapshot");
        atlas_core::compile(&atlas_core::CompileOptions::new(&db, &snap)).expect("compile");
        let opened = atlas_core::Snapshot::open(&snap).unwrap();
        atlas_layout::compute(&opened, &atlas_layout::LayoutParams::default())
            .unwrap()
            .write(&tmp.join("layout"))
            .unwrap();
        let mut cfg = ServerConfig::new(&snap);
        cfg.db = Some(db);
        cfg.layout_dir = Some(tmp.join("layout"));
        cfg.web_dir = tmp.join("web");
        router(AppState::load(&cfg).expect("load"))
    })
}

async fn json(uri: &str) -> (StatusCode, Value) {
    let res = app().clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
    let status = res.status();
    let b = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&b).unwrap_or_else(|e| panic!("{uri}: {e}")))
}

#[tokio::test]
async fn concept_pages_members_with_their_evidence() {
    let (s, v) = json("/api/concept?id=c:lonely&limit=2").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["concept"]["id"], "c:lonely");
    assert_eq!(v["members_total"], 2, "sas-congruence and parallelogram-area are primary");
    assert_eq!(v["memberships_total"], 3, "pythagoras is a secondary member");
    assert_eq!(v["memberships_by_state"]["model_proposed"], 3);
    assert_eq!(v["truncated"], true);
    let m = v["members"].as_array().unwrap();
    assert_eq!(m.len(), 2);
    assert!(m.iter().all(|x| x["primary"] == true), "primary members come first");
    let sas = m.iter().find(|x| x["id"] == "s:sas-congruence").expect("sas on the first page");
    assert_eq!(sas["signal"], "zone");
    assert_eq!(sas["claim_id"], "c:m3");
    assert_eq!(sas["score"]["calibration"], "not calibrated");
    assert_eq!(sas["provenance"][0]["extractor"], "concept-projection");
    let (_, p2) = json("/api/concept?id=c:lonely&limit=2&offset=2").await;
    assert_eq!(p2["members"][0]["id"], "s:pythagoras");
    assert_eq!(p2["members"][0]["primary"], false);
    assert_eq!(p2["truncated"], false);
}

#[tokio::test]
async fn concept_input_is_validated() {
    for (uri, code, status) in [
        ("/api/concept?id=c:lonely&limit=0", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept?id=c:lonely&limit=201", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept?id=c:lonely&offset=1000001", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept?id=c:lonely&edge_limit=0", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept?id=c:lonely&lang=spanish", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept?id=%3Cx%3E", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept?id=c:lonely&x=1", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept?id=s:pythagoras", "not_found", StatusCode::NOT_FOUND),
        ("/api/concept?id=nope:nope", "not_found", StatusCode::NOT_FOUND),
        ("/api/concept/edges?from=c:lonely&to=c:right-triangle&relation=nope", "bad_request", StatusCode::BAD_REQUEST),
        ("/api/concept/edges?from=c:lonely", "bad_request", StatusCode::BAD_REQUEST),
    ] {
        let (s, v) = json(uri).await;
        assert_eq!(s, status, "{uri}: {v}");
        assert_eq!(v["error"]["code"], code, "{uri}");
    }
}

#[tokio::test]
async fn drill_down_recounts_each_aggregated_edge() {
    let mut checked = 0;
    for (a, b) in [("c:lonely", "c:right-triangle"), ("c:right-triangle", "c:lonely")] {
        let (_, v) = json(&format!("/api/concept?id={a}")).await;
        for e in v["edges_out"].as_array().unwrap() {
            assert_eq!(e["concept"]["id"], b);
            let (s, d) = json(&format!("/api/concept/edges?from={a}&to={b}&limit=200")).await;
            assert_eq!(s, StatusCode::OK);
            assert_eq!(d["total"], e["total"], "{a} -> {b}");
            assert_eq!(d["edges"].as_array().unwrap().len() as u64, e["total"].as_u64().unwrap());
            for x in d["edges"].as_array().unwrap() {
                assert_ne!(x["relation"], "member_of_concept");
            }
            let (_, rev) = json(&format!("/api/concept?id={b}")).await;
            assert!(
                rev["edges_in"].as_array().unwrap().iter().any(|i| i["concept"]["id"] == a && i["total"] == e["total"]),
                "{a} -> {b} listed as incoming of {b}"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "the fixture has aggregated edges between its concepts");
}

#[tokio::test]
async fn entity_links_the_concept_layer_without_listing_every_membership() {
    let (_, v) = json("/api/entity/c:lonely").await;
    assert_eq!(v["concept"]["as_concept"]["members"], 2);
    assert_eq!(v["counts_by_relation"]["member_of_concept"], 3);
    assert!(
        v["claims"].as_array().unwrap().iter().all(|c| c["relation"] != "member_of_concept"),
        "memberships of a concept are paged by /api/concept, not listed in the entity"
    );
    let (_, d) = json("/api/entity/s:pythagoras").await;
    assert_eq!(d["concept"]["primary_concept"]["id"], "c:right-triangle");
    assert!(d["claims"].as_array().unwrap().iter().any(|c| c["relation"] == "member_of_concept"), "a declaration keeps its own");
    let (_, h) = json("/api/health").await;
    assert_eq!(h["concepts"], 2);
    assert_eq!(h["map"]["general_zoom"], 2);
    let (_, t) = json("/api/map/tiles/0/0/0").await;
    let kinds: Vec<&str> = t["nodes"].as_array().unwrap().iter().map(|n| n["kind"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"concept") && !kinds.contains(&"cluster"), "{kinds:?}");
}
