//! HTTP contract of SPEC §15 over the mini_atlas fixture (registry + compiled snapshot).

use atlas_server::{AppState, ServerConfig, router};
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::OnceLock;
use tower::ServiceExt;

/// Project root, through junctions (canonical path without the `\?\` prefix).
fn root() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).canonicalize().unwrap();
    PathBuf::from(dir.to_string_lossy().trim_start_matches(r"\\?\")).join("../..")
}

/// Snapshot of the fixture registry + a tiny web dir, built once per test binary.
fn fixture() -> &'static (PathBuf, PathBuf, PathBuf) {
    static F: OnceLock<(PathBuf, PathBuf, PathBuf)> = OnceLock::new();
    // (registry, snapshot dir, web dir); the layout, if any, is next to the snapshot.
    F.get_or_init(|| {
        let db = root().join("core/atlas-core/tests/fixtures/mini_atlas.db");
        let tmp = std::env::temp_dir().join(format!("atlas-server-api-{}", std::process::id()));
        let snap = tmp.join("snapshot");
        atlas_core::compile(&atlas_core::CompileOptions::new(&db, &snap)).expect("compile fixture");
        // Layout if atlas-layout is implemented; otherwise tiles answer not_implemented.
        let opened = atlas_core::Snapshot::open(&snap).expect("open fixture snapshot");
        let _ = atlas_layout::compute(&opened, &atlas_layout::LayoutParams::default())
            .and_then(|l| l.write(&tmp.join("layout")));
        let web = tmp.join("web");
        std::fs::create_dir_all(web.join("assets")).unwrap();
        std::fs::write(web.join("index.html"), "<!doctype html><title>atlas</title>").unwrap();
        std::fs::write(web.join("assets/app-123.js"), "console.log(1)".repeat(200)).unwrap();
        std::fs::write(tmp.join("secret.txt"), "do not serve").unwrap();
        (db, snap, web)
    })
}

fn app_with(cors: Option<&str>) -> Router {
    let (db, snap, web) = fixture();
    let mut cfg = ServerConfig::new(snap);
    cfg.db = Some(db.clone());
    cfg.web_dir = web.clone();
    cfg.composition = Some(root().join("shapes/composition.json"));
    cfg.layout_dir = Some(snap.parent().unwrap().join("layout"));
    cfg.cors_origin = cors.map(String::from);
    router(AppState::load(&cfg).expect("load state"))
}

fn app() -> Router {
    app_with(None)
}

async fn get(app: &Router, uri: &str, headers: &[(&str, &str)]) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::get(uri);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let res = app.clone().oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    let (parts, body) = res.into_parts();
    (parts.status, parts.headers, body.collect().await.unwrap().to_bytes().to_vec())
}

async fn json(app: &Router, uri: &str) -> (StatusCode, Value) {
    let (s, _, b) = get(app, uri, &[]).await;
    (s, serde_json::from_slice(&b).unwrap_or_else(|e| panic!("{uri}: {e}: {}", String::from_utf8_lossy(&b))))
}

fn assert_envelope(v: &Value, code: &str) {
    assert_eq!(v["error"]["code"], code, "{v}");
    assert!(v["error"]["message"].as_str().is_some_and(|m| !m.is_empty()), "{v}");
}

#[tokio::test]
async fn health_reports_components() {
    let (s, v) = json(&app(), "/api/health").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["status"], "ok");
    assert_eq!(v["snapshot"]["nodes"], 19);
    assert!(v["components"]["registry"].as_str().unwrap().starts_with("ok"), "{v}");
    assert!(v["components"]["search"].is_string());
}

#[tokio::test]
async fn search_is_accent_insensitive_prefix_and_resolves_merges() {
    let app = app();
    let (s, v) = json(&app, "/api/search?q=teorema%20de%20pitag&lang=es").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["hits"][0]["id"], "s:pythagoras", "{v}");
    assert_eq!(v["hits"][0]["type"], "statement");
    assert_eq!(v["hits"][0]["lanes"]["lexical"]["rank"], 1);
    assert_eq!(v["limit"], 20);
    // The merged duplicate's English label finds the survivor, once.
    let (_, v) = json(&app, "/api/search?q=duplicate&lang=en").await;
    let ids: Vec<&str> = v["hits"].as_array().unwrap().iter().map(|h| h["id"].as_str().unwrap()).collect();
    assert_eq!(ids.iter().filter(|i| **i == "s:pythagoras").count(), 1, "{v}");
    assert!(!ids.contains(&"s:pythagoras-dup"), "{v}");
    // Type filter.
    let (_, v) = json(&app, "/api/search?q=pitag&types=concept").await;
    assert!(v["hits"].as_array().unwrap().iter().all(|h| h["type"] == "concept"), "{v}");
}

#[tokio::test]
async fn bad_parameters_get_the_json_envelope() {
    let app = app();
    for uri in [
        "/api/search",
        "/api/search?q=",
        "/api/search?q=x&limit=0",
        "/api/search?q=x&limit=101",
        "/api/search?q=x&lang=spanish",
        "/api/search?q=x&types=planet",
        "/api/search?q=x&unknown=1",
        "/api/neighborhood?id=s:pythagoras&depth=9",
        "/api/neighborhood?id=s:pythagoras&states=retracted",
        "/api/neighborhood?id=bad%20id",
        "/api/path?from=s:pythagoras&to=s:pythagoras&max_hops=99",
        "/api/path?from=s:pythagoras&to=s:pythagoras&objective=fastest",
        "/api/path?from=s:pythagoras&to=s:pythagoras&weights=evidence:-1",
        "/api/map/tiles?z=1&x=2&y=0",
        "/api/map/tiles/30/0/0",
        "/api/map/tiles?z=0&x=0&y=0&layer=moon",
        "/api/map/tiles?z=0&x=0&y=0&format=png",
    ] {
        let (s, v) = json(&app, uri).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{uri}: {v}");
        assert_envelope(&v, "bad_request");
    }
    let long = format!("/api/search?q={}", "a".repeat(3000));
    let (s, v) = json(&app, &long).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_envelope(&v, "bad_request");
    for uri in ["/api/entity/s:nope", "/api/neighborhood?id=s:nope", "/api/nothing-here"] {
        let (s, v) = json(&app, uri).await;
        assert_eq!(s, StatusCode::NOT_FOUND, "{uri}: {v}");
        assert_envelope(&v, "not_found");
    }
}

#[tokio::test]
async fn entity_detail_resolves_merges_and_labels_participants() {
    let app = app();
    let (s, v) = json(&app, "/api/entity/s:pythagoras-dup?lang=es").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["canonical_id"], "s:pythagoras");
    assert_eq!(v["merged_from"], serde_json::json!(["s:pythagoras-dup"]));
    assert_eq!(v["coverage"], "known");
    assert_eq!(v["entity"]["id"], "s:pythagoras");
    assert!(v["entity"]["aliases"].is_array() && v["entity"]["formula"].is_string() || v["entity"]["formula"].is_null());
    // Two alternative proofs (subject) + the converse that uses it as a premise.
    assert_eq!(v["counts_by_relation"]["depends_on_logically"], 3, "{v}");
    let claims = v["claims"].as_array().unwrap();
    let euclid = claims.iter().find(|c| c["id"] == "c:pyth-euclid").expect("conjunction listed");
    assert_eq!(euclid["alternative_group"], "g:pythagoras-proofs");
    assert_eq!(euclid["labels"]["s:sas-congruence"].as_str().map(|s| !s.is_empty()), Some(true), "{euclid}");
    assert!(euclid["provenance"][0].get("quote").is_some());
    // ADR-0005 §7f: only a formalized_as proposed by name is hidden from explore
    assert!(claims.iter().all(|c| c["explore_hidden"] == (c["relation"] == "formalized_as" && c["evidence_state"] == "model_proposed")), "{v}");
    // Superseded versions are not current: only the v2 of the cosines claim shows.
    assert!(claims.iter().all(|c| c["id"] != "c:cosines-gen"), "{v}");
    assert!(claims.iter().any(|c| c["id"] == "c:cosines-gen-v2"));
    assert_eq!(v["sources"]["src:fixture-hand"]["license"], "CC0-1.0");
    // Coverage says unknown, never isolated.
    let (_, v) = json(&app, "/api/entity/c:lonely").await;
    assert_eq!(v["coverage"], "unknown");
    assert_eq!(v["claims"], serde_json::json!([]));
    // Limit and truncation.
    let (_, v) = json(&app, "/api/entity/s:pythagoras?limit=1").await;
    assert_eq!(v["claims"].as_array().unwrap().len(), 1);
    assert_eq!(v["truncated"], true);
    // Paging: the pages of `limit=1` are the claims of one full answer, in order, without repeats.
    let (_, all) = json(&app, "/api/entity/s:pythagoras").await;
    let all: Vec<Value> = all["claims"].as_array().unwrap().iter().map(|c| c["id"].clone()).collect();
    assert!(all.len() >= 2, "{all:?}");
    let mut paged = Vec::new();
    for offset in 0..all.len() {
        let (s, v) = json(&app, &format!("/api/entity/s:pythagoras?limit=1&offset={offset}")).await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["offset"], offset);
        assert_eq!(v["truncated"], offset + 1 < all.len(), "{v}");
        paged.extend(v["claims"].as_array().unwrap().iter().map(|c| c["id"].clone()));
    }
    assert_eq!(paged, all);
    let (s, _) = json(&app, "/api/entity/s:pythagoras?offset=99999999").await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn neighborhood_keeps_conjunctions_and_counts_what_it_hides() {
    let app = app();
    let (s, v) = json(&app, "/api/neighborhood?id=s:pythagoras&depth=1").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["center"], "s:pythagoras");
    let edges = v["edges"].as_array().unwrap();
    let euclid: Vec<&Value> = edges.iter().filter(|e| e["claim_id"] == "c:pyth-euclid").collect();
    assert_eq!(euclid.len(), 2, "one edge per premise: {v}");
    assert!(euclid.iter().all(|e| e["conjunct_count"] == 2 && e["alternative_group"] == "g:pythagoras-proofs"));
    assert_eq!(v["truncated"], false);
    let node_ids: Vec<&str> = v["nodes"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
    for e in edges {
        assert!(node_ids.contains(&e["source"].as_str().unwrap()) && node_ids.contains(&e["target"].as_str().unwrap()));
    }
    // A node limit of 1 hides every edge and says how many, per relation.
    let (_, v) = json(&app, "/api/neighborhood?id=s:pythagoras&limit=1").await;
    assert_eq!(v["truncated"], true);
    assert_eq!(v["edges"], serde_json::json!([]));
    let hidden: u64 = v["hidden"].as_object().unwrap().values().map(|n| n.as_u64().unwrap()).sum();
    assert_eq!(hidden as usize, edges.len());
    // Relation filter: only that relation is drawn; the rest is hidden, not dropped.
    let (_, v) = json(&app, "/api/neighborhood?id=s:pythagoras&relations=generalizes").await;
    assert!(v["edges"].as_array().unwrap().iter().all(|e| e["relation"] == "generalizes"), "{v}");
    assert!(v["hidden"]["depends_on_logically"].as_u64().unwrap() >= 2, "{v}");
}

#[tokio::test]
async fn coverage_matches_store_semantics() {
    let (s, v) = json(&app(), "/api/coverage").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["active_merges"], 1);
    assert_eq!(v["unknown_by_type"]["concept"], 1);
    assert_eq!(v["claims_by_source"]["src:fixture-hand"], v["claims_by_state"].as_object().unwrap().values().map(|n| n.as_u64().unwrap()).sum::<u64>());
    assert!(v["generated_at"].as_str().unwrap().ends_with('Z'));
}

#[tokio::test]
async fn etag_short_circuits_with_304() {
    let app = app();
    let (s, h, _) = get(&app, "/api/neighborhood?id=s:pythagoras", &[]).await;
    assert_eq!(s, StatusCode::OK);
    let tag = h.get(header::ETAG).expect("etag").to_str().unwrap().to_string();
    assert!(tag.starts_with("W/\""));
    let (s, _, body) = get(&app, "/api/neighborhood?id=s:pythagoras", &[("if-none-match", &tag)]).await;
    assert_eq!(s, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
    let (s, h2, _) = get(&app, "/api/neighborhood?id=s:pythagoras&depth=2", &[("if-none-match", &tag)]).await;
    assert_eq!(s, StatusCode::OK);
    assert_ne!(h2.get(header::ETAG).unwrap(), tag.as_str());
}

#[tokio::test]
async fn responses_compress_on_request() {
    let app = app();
    let (s, h, _) = get(&app, "/api/coverage", &[("accept-encoding", "br")]).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get(header::CONTENT_ENCODING).map(|v| v.to_str().unwrap()), Some("br"));
    let (_, h, _) = get(&app, "/api/coverage", &[("accept-encoding", "gzip")]).await;
    assert_eq!(h.get(header::CONTENT_ENCODING).map(|v| v.to_str().unwrap()), Some("gzip"));
    let (_, h, _) = get(&app, "/api/coverage", &[]).await;
    assert!(h.get(header::CONTENT_ENCODING).is_none());
}

#[tokio::test]
async fn static_files_are_served_without_traversal() {
    let app = app();
    let (s, h, b) = get(&app, "/", &[]).await;
    assert_eq!(s, StatusCode::OK);
    assert!(String::from_utf8_lossy(&b).contains("<title>atlas</title>"));
    assert_eq!(h.get(header::CACHE_CONTROL).unwrap(), "no-cache");
    assert!(h.get(header::CONTENT_SECURITY_POLICY).is_some());
    assert_eq!(h.get(header::X_CONTENT_TYPE_OPTIONS).unwrap(), "nosniff");
    let (s, h, _) = get(&app, "/assets/app-123.js", &[]).await;
    assert_eq!(s, StatusCode::OK);
    assert!(h.get(header::CACHE_CONTROL).unwrap().to_str().unwrap().contains("immutable"));
    for uri in ["/../secret.txt", "/..%2fsecret.txt", "/%2e%2e/secret.txt", "/assets/..%5c..%5csecret.txt", "/assets/../../secret.txt"] {
        let (s, _, b) = get(&app, uri, &[]).await;
        assert!(!String::from_utf8_lossy(&b).contains("do not serve"), "{uri} leaked");
        assert_ne!(s, StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn cors_is_off_by_default_and_exact_when_configured() {
    let (_, h, _) = get(&app(), "/api/health", &[("origin", "https://evil.example")]).await;
    assert!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
    let app = app_with(Some("http://localhost:5173"));
    let (_, h, _) = get(&app, "/api/health", &[("origin", "http://localhost:5173")]).await;
    assert_eq!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), "http://localhost:5173");
    let (_, h, _) = get(&app, "/api/health", &[("origin", "https://evil.example")]).await;
    assert_ne!(h.get(header::ACCESS_CONTROL_ALLOW_ORIGIN).map(|v| v.to_str().unwrap()), Some("https://evil.example"));
}

#[tokio::test]
async fn only_get_and_head() {
    let app = app();
    let res = app
        .clone()
        .oneshot(Request::post("/api/health").body(Body::from("x")).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    let res = app.oneshot(Request::head("/api/health").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn path_explains_each_step_and_validates_options() {
    let app = app();
    let (s, v) = json(&app, "/api/path?from=c:right-triangle&to=s:pythagoras&k=2").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["objective"], "best_supported");
    assert!(v["weights"].is_object());
    if v["abstained"] == false {
        let p = &v["paths"][0];
        assert!(p["conclusion"]["authorizes"].is_string() && p["conclusion"]["strength"].is_u64(), "{p}");
        let step = &p["steps"][0];
        assert_eq!(step["from"], "c:right-triangle", "{p}");
        assert!(step["from_label"].is_string() && step["authorizes"].is_string() && step["sources"].is_array(), "{step}");
    } else {
        assert!(v["reason"].is_string(), "abstention must say why: {v}");
    }
    for uri in [
        "/api/path?from=c:right-triangle&to=s:pythagoras&k=9",
        "/api/path?from=c:right-triangle&to=s:pythagoras&min_strength=6",
        "/api/path?from=c:right-triangle&to=s:pythagoras&known=bad%20id",
        "/api/path?from=c:right-triangle&to=s:pythagoras&weights=nonsense:1",
    ] {
        let (s, v) = json(&app, uri).await;
        assert_eq!(s, StatusCode::BAD_REQUEST, "{uri}: {v}");
        assert_envelope(&v, "bad_request");
    }
    let (s, v) = json(&app, "/api/path?from=c:right-triangle&to=s:nope").await;
    assert_eq!(s, StatusCode::NOT_FOUND, "{v}");
}

#[tokio::test]
async fn tiles_come_in_json_and_binary_with_the_same_content() {
    let app = app();
    let (s, v) = json(&app, "/api/map/tiles?layer=structure&z=0&x=0&y=0").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!((v["z"].as_u64(), v["layer"].as_str()), (Some(0), Some("structure")));
    let nodes = v["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty(), "{v}");
    assert!(nodes.iter().all(|n| (0.0..=1.0).contains(&n["x"].as_f64().unwrap())), "{v}");
    let max_zoom = v["max_zoom"].as_u64().unwrap();
    let (s, h, b) = get(&app, "/api/map/tiles/0/0/0?format=bin", &[]).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(h.get(header::CONTENT_TYPE).unwrap(), atlas_server::TILE_MIME);
    assert_eq!(&b[..4], atlas_server::TILE_MAGIC);
    let n_nodes = u32::from_le_bytes(b[48..52].try_into().unwrap());
    assert_eq!(n_nodes as usize, nodes.len());
    let beyond = format!("/api/map/tiles/{}/0/0", max_zoom + 1);
    let (s, v) = json(&app, &beyond).await;
    assert_eq!(s, StatusCode::BAD_REQUEST, "{beyond}: {v}");
    // Region focus: where an entity sits, coarse to fine communities.
    let (s, v) = json(&app, "/api/map/locate?id=s:pythagoras-dup").await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["id"], "s:pythagoras");
    assert!(v["communities"].as_array().is_some_and(|c| !c.is_empty()), "{v}");
}
