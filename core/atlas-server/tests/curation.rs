//! ADR-0005 §5 in the API: a presentation label is the label the server sends (the registry label
//! is untouched), a `search:default` node is never a search hit, and a `map:all` node is not drawn
//! in a neighborhood.

use atlas_server::{AppState, ServerConfig, router};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;

#[tokio::test]
async fn server_reads_presentation_labels_and_node_flags() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../atlas-core/tests/fixtures");
    let tmp = std::env::temp_dir().join(format!("atlas-server-curation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("web")).unwrap();
    std::fs::write(tmp.join("web/index.html"), "<!doctype html><title>atlas</title>").unwrap();
    let db = tmp.join("atlas.db");
    std::fs::copy(fixtures.join("mini_atlas.db"), &db).unwrap();
    rusqlite::Connection::open(&db)
        .unwrap()
        .execute_batch(
            r#"CREATE TABLE curation(id INTEGER PRIMARY KEY, kind TEXT NOT NULL, target_id TEXT NOT NULL,
                 rule TEXT NOT NULL, rule_version TEXT NOT NULL, payload TEXT NOT NULL, actor TEXT NOT NULL,
                 reason TEXT NOT NULL, created_at TEXT NOT NULL, undone_at TEXT, undone_by TEXT, undo_reason TEXT);
               INSERT INTO curation(kind, target_id, rule, rule_version, payload, actor, reason, created_at) VALUES
                 ('presentation_label', 's:pythagoras', 'test', '1', '{"lang":"es","text":"Pitágoras (curado)","provenance":{}}', 'curation:test/1', 'test', 't'),
                 ('exclude_presentation', 's:sas-congruence', 'test', '1', '{"scopes":["map:all","search:default"]}', 'curation:test/1', 'test', 't');
               PRAGMA user_version = 4;"#,
        )
        .unwrap();
    atlas_core::compile(&atlas_core::CompileOptions::new(&db, tmp.join("snapshot"))).expect("compile");
    let mut cfg = ServerConfig::new(tmp.join("snapshot"));
    cfg.db = Some(db);
    cfg.web_dir = tmp.join("web");
    let app = router(AppState::load(&cfg).expect("load state"));
    let get = |uri: &'static str| {
        let app = app.clone();
        async move {
            let res = app.oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
            let status = res.status();
            let body = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&body).unwrap())
        }
    };
    let (s, v) = get("/api/entity/s:pythagoras?lang=es").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["label"], "Pitágoras (curado)", "{v}");
    assert_eq!(v["entity"]["labels"]["es"], "Teorema de Pitágoras", "registry label untouched: {v}");
    let (_, v) = get("/api/search?q=pitag&lang=es").await;
    assert!(v["hits"].as_array().unwrap().iter().any(|h| h["label"] == "Pitágoras (curado)"), "{v}");
    let (_, v) = get("/api/search?q=congruencia&lang=es").await;
    assert!(v["hits"].as_array().unwrap().iter().all(|h| h["id"] != "s:sas-congruence"), "{v}");
    let (_, v) = get("/api/neighborhood?id=s:pythagoras&depth=1&limit=500").await;
    assert!(v["nodes"].as_array().unwrap().iter().all(|n| n["id"] != "s:sas-congruence"), "{v}");
    assert!(v["nodes"].as_array().unwrap().iter().any(|n| n["label"] == "Pitágoras (curado)"), "{v}");
}
