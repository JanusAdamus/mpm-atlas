//! HTTP plumbing shared by every handler: the SPEC §15 error envelope, a strict
//! query-string extractor, the bounded blocking pool, ETags and response headers.

use atlas_core::Error;
use axum::body::Body;
use axum::extract::{FromRequestParts, Request, State};
use axum::http::header::{self, HeaderMap, HeaderValue};
use axum::http::request::Parts;
use axum::http::{StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;
use std::sync::Arc;
use std::time::Duration;

use crate::{AppState, Engine};

/// Longest accepted query string; every parameter of SPEC §15 fits well below it.
pub const MAX_QUERY_BYTES: usize = 2048;
/// How long a request waits for a slot in the heavy-work pool before a 503.
const BUSY_WAIT: Duration = Duration::from_secs(2);

/// SPEC §15 error envelope `{"error": {"code", "message"}}` with its HTTP status.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        ApiError { status: StatusCode::BAD_REQUEST, code: "bad_request", message: msg.into() }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        ApiError { status: StatusCode::NOT_FOUND, code: "not_found", message: msg.into() }
    }

    fn busy() -> Self {
        ApiError { status: StatusCode::SERVICE_UNAVAILABLE, code: "busy", message: "server busy, retry".into() }
    }
}

impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        let status = match e.code() {
            "not_found" => StatusCode::NOT_FOUND,
            "bad_request" => StatusCode::BAD_REQUEST,
            "not_implemented" => StatusCode::NOT_IMPLEMENTED,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            eprintln!("atlas-server: internal error: {e}");
        }
        ApiError { status, code: e.code(), message: e.to_string() }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({"error": {"code": self.code, "message": self.message}});
        (self.status, axum::Json(body)).into_response()
    }
}

/// Query string deserialized strictly: unknown parameters, bad types and oversized
/// strings are a 400 with the JSON envelope (axum's own `Query` answers plain text).
pub struct Q<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Q<T> {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        let q = parts.uri.query().unwrap_or("");
        if q.len() > MAX_QUERY_BYTES {
            return Err(ApiError::bad_request(format!("query string longer than {MAX_QUERY_BYTES} bytes")));
        }
        serde_urlencoded::from_str(q).map(Q).map_err(|e| ApiError::bad_request(e.to_string()))
    }
}

/// Run CPU- or IO-bound work on the blocking pool, at most `heavy_permits` at once.
/// A request that cannot get a slot within `BUSY_WAIT` is shed with 503.
pub async fn blocking<T, F>(engine: &Arc<Engine>, f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&Engine) -> atlas_core::Result<T> + Send + 'static,
{
    let permit = tokio::time::timeout(BUSY_WAIT, engine.heavy.clone().acquire_owned())
        .await
        .map_err(|_| ApiError::busy())?
        .map_err(|_| ApiError::busy())?;
    let engine = engine.clone();
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        f(&engine)
    })
    .await
    .map_err(|e| ApiError::from(Error::Registry(format!("worker panicked: {e}"))))?
    .map_err(ApiError::from)
}

/// `200 application/json` from bytes already serialized.
pub fn json_bytes(body: Vec<u8>) -> Response {
    ([(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))], body).into_response()
}

pub fn to_json_vec<T: serde::Serialize>(v: &T) -> atlas_core::Result<Vec<u8>> {
    serde_json::to_vec(v).map_err(|e| Error::Registry(format!("serialize response: {e}")))
}

/// Weak ETag of a snapshot-derived response: hash of (snapshot hash, layout version, URI).
/// Weak because compression changes the bytes but not the meaning.
pub fn etag_for(seed: &[u8; 32], uri: &Uri) -> HeaderValue {
    let mut h = blake3::Hasher::new();
    h.update(seed);
    h.update(uri.path_and_query().map_or(uri.path(), |p| p.as_str()).as_bytes());
    let hex = h.finalize().to_hex();
    HeaderValue::from_str(&format!("W/\"{}\"", &hex[..32])).expect("hex is a valid header")
}

fn matches_if_none_match(headers: &HeaderMap, tag: &HeaderValue) -> bool {
    let Some(inm) = headers.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) else { return false };
    let tag = tag.to_str().unwrap_or_default();
    let bare = tag.trim_start_matches("W/");
    inm.split(',').map(str::trim).any(|t| t == "*" || t.trim_start_matches("W/") == bare)
}

/// ETag + conditional GET for routes whose answer is a pure function of the snapshot,
/// the layout and the URI.  A matching `If-None-Match` short-circuits before any work.
pub async fn etag(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let tag = etag_for(&st.engine.etag_seed, req.uri());
    if matches_if_none_match(req.headers(), &tag) {
        let mut res = StatusCode::NOT_MODIFIED.into_response();
        res.headers_mut().insert(header::ETAG, tag);
        return res;
    }
    let mut res = next.run(req).await;
    if res.status() == StatusCode::OK {
        res.headers_mut().insert(header::ETAG, tag);
        res.headers_mut().insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    }
    res
}

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; \
                   font-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";

/// Security headers on everything; cache policy on static files (hashed assets are immutable).
pub async fn headers(req: Request, next: Next) -> Response {
    let is_asset = req.uri().path().starts_with("/assets/");
    let is_api = req.uri().path().starts_with("/api/");
    let mut res: Response<Body> = next.run(req).await;
    let h = res.headers_mut();
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(header::REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(CSP));
    if !is_api && !h.contains_key(header::CACHE_CONTROL) {
        let policy = if is_asset { "public, max-age=31536000, immutable" } else { "no-cache" };
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static(policy));
    }
    res
}

/// Two-letter language tag (`es`, `en`, …): labels fall back per `Entity.label`.
pub fn check_lang(lang: &str) -> Result<(), ApiError> {
    if lang.len() == 2 && lang.bytes().all(|b| b.is_ascii_lowercase()) {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!("lang must be a two-letter code, got {lang:?}")))
    }
}

/// Entity / claim id as in `atlas.model` (`^[A-Za-z0-9][A-Za-z0-9_.:\-/#]{0,255}$`).
pub fn check_id(id: &str) -> Result<(), ApiError> {
    let ok = !id.is_empty()
        && id.len() <= 256
        && id.as_bytes()[0].is_ascii_alphanumeric()
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"_.:-/#".contains(&b));
    if ok { Ok(()) } else { Err(ApiError::bad_request(format!("malformed id {id:?}"))) }
}

/// `limit` parameter: default when absent, 400 outside `1..=max`.
pub fn check_limit(limit: Option<usize>, default: usize, max: usize) -> Result<usize, ApiError> {
    match limit.unwrap_or(default) {
        n if (1..=max).contains(&n) => Ok(n),
        n => Err(ApiError::bad_request(format!("limit must be in 1..={max}, got {n}"))),
    }
}

/// Comma-separated list of vocabulary names (`relations=cites,generalizes`).
pub fn parse_list<T: std::str::FromStr>(raw: Option<&str>, what: &str) -> Result<Vec<T>, ApiError>
where
    T::Err: std::fmt::Display,
{
    raw.unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.parse::<T>().map_err(|e| ApiError::bad_request(format!("{what}: {e}"))))
        .collect()
}

pub async fn api_not_found(uri: Uri) -> ApiError {
    ApiError::not_found(format!("no API route {}", uri.path()))
}
