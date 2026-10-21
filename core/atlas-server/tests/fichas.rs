//! r5 fichas over the mini_atlas fixture plus one note (atlas-core/tests/fixtures/note.sql):
//! `/api/entity` carries the ficha's `explains` sentences on the first page whatever the claim
//! limit, with the note's label in `labels`.

use atlas_server::{AppState, ServerConfig, router};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use std::path::PathBuf;
use tower::ServiceExt;

#[tokio::test]
async fn explains_ride_past_the_claim_cap_with_the_note_label() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../atlas-core/tests/fixtures");
    let tmp = std::env::temp_dir().join(format!("atlas-server-fichas-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("web")).unwrap();
    std::fs::write(tmp.join("web/index.html"), "<!doctype html><title>atlas</title>").unwrap();
    let db = tmp.join("atlas.db");
    std::fs::copy(fixtures.join("mini_atlas.db"), &db).unwrap();
    rusqlite::Connection::open(&db).unwrap().execute_batch(include_str!("../../atlas-core/tests/fixtures/note.sql")).unwrap();
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
    // limit=1: the page holds one claim; the explains (id sorts last) still comes on page 1
    let (s, v) = get("/api/entity/s:pythagoras?lang=es&limit=1").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["truncated"], true, "{v}");
    let claims = v["claims"].as_array().unwrap();
    let ex = claims.iter().find(|c| c["id"] == "c:zz-explains-pyth").expect("explains past the cap");
    assert_eq!(ex["labels"]["note:que_es:fixture1"], "Zanahoria: frase de ficha que explica el teorema.");
    assert_eq!(claims.iter().filter(|c| c["id"] == "c:zz-explains-pyth").count(), 1, "no duplicate");
    // a note is never a connection: the neighborhood leaves it out (and counts the explains as hidden)
    let (s, v) = get("/api/neighborhood?id=s:pythagoras&depth=2&limit=500").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert!(v["nodes"].as_array().unwrap().iter().all(|n| n["type"] != "note"), "{v}");
    assert!(v["edges"].as_array().unwrap().iter().all(|e| e["relation"] != "explains"), "{v}");
    assert_eq!(v["hidden"]["explains"], 1, "{v}");
    // later pages do not repeat it
    let (_, v) = get("/api/entity/s:pythagoras?lang=es&limit=1&offset=1").await;
    assert!(v["claims"].as_array().unwrap().iter().all(|c| c["id"] != "c:zz-explains-pyth"), "{v}");
}
