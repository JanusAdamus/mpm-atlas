//! `GET /api/map/tiles?layer=&z=&x=&y=` (SPEC §15) and `/api/map/tiles/{z}/{x}/{y}`.
//! JSON by default; `format=bin` returns the compact columnar encoding below, which
//! the web client maps straight into typed arrays (no JSON parse, no per-node objects).
//!
//! Binary tile v1 (little-endian, every section 4-byte aligned; NONE = u32::MAX):
//! ```text
//! magic "ATT1" | z x y max_zoom: u32 | bbox: f32x4 | layer: u32 (Projection code) | layout_version: u32 (string)
//! n_strings n_nodes n_external n_edges n_aggs n_agg_rel n_hidden n_central: u32
//! str_off: u32x(n_strings+1) | str_bytes: UTF-8, padded
//! items (N = n_nodes + n_external; nodes first, then external endpoints):
//!   x f32xN | y f32xN | size u32xN | id u32xN | label u32xN | community u32xN | priority u32xN
//!   central_off u32x(N+1) | central u32 x n_central (strings) | kind u8xN (0 entity, 1 cluster, 2 concept)
//!   | type u8xN (255 = none), padded
//! edges:  source u32xe | target u32xe (item indexes) | count u32xe | relation u8xe | state u8xe, padded
//! aggs:   item u32xa | members u32xa | hidden_members u32xa | rel_off u32x(a+1) | rel_count u32 x n_agg_rel | rel u8 x n_agg_rel, padded
//! hidden: count u32 x n_hidden | relation u8 x n_hidden, padded   (tile edges over budget, per relation)
//! ```
//! String fields are indexes into the deduplicated string table.

use atlas_core::{EntityType, NONE, Projection};
use atlas_layout::{TileNode, TileNodeKind, TileResponse};
use axum::extract::{Path as UrlPath, State};
use axum::http::HeaderValue;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::collections::HashMap;

use crate::http::{ApiError, Q, blocking, check_lang, json_bytes, to_json_vec};
use crate::{AppState, Engine};

pub const TILE_MIME: &str = "application/vnd.atlas.tile";
pub const MAGIC: &[u8; 4] = b"ATT1";
pub const MAX_ZOOM: u32 = 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    layer: Option<String>,
    z: Option<u32>,
    x: Option<u32>,
    y: Option<u32>,
    lang: Option<String>,
    format: Option<String>,
    /// Client cache-buster (layout version); part of the URI, hence of the ETag.
    v: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Bin,
}

struct TileQuery {
    layer: Projection,
    z: u32,
    x: u32,
    y: u32,
    lang: String,
    format: Format,
}

fn parse(p: Params, zxy: Option<(u32, u32, u32)>) -> Result<TileQuery, ApiError> {
    let (z, x, y) = match zxy {
        Some(t) => t,
        None => match (p.z, p.x, p.y) {
            (Some(z), Some(x), Some(y)) => (z, x, y),
            _ => return Err(ApiError::bad_request("z, x and y are required")),
        },
    };
    if z > MAX_ZOOM || x >= 1 << z || y >= 1 << z {
        return Err(ApiError::bad_request(format!("tile {z}/{x}/{y} outside 0 <= x, y < 2^z, z <= {MAX_ZOOM}")));
    }
    let layer = match p.layer.as_deref() {
        None => Projection::Structure,
        Some(s) => s.parse().map_err(|e| ApiError::bad_request(format!("layer: {e}")))?,
    };
    let lang = p.lang.unwrap_or_else(|| "es".into());
    check_lang(&lang)?;
    let format = match p.format.as_deref() {
        None | Some("json") => Format::Json,
        Some("bin") => Format::Bin,
        Some(f) => return Err(ApiError::bad_request(format!("format must be json or bin, got {f:?}"))),
    };
    if p.v.as_ref().is_some_and(|v| v.len() > 64) {
        return Err(ApiError::bad_request("v is at most 64 characters"));
    }
    Ok(TileQuery { layer, z, x, y, lang, format })
}

pub async fn by_query(st: State<AppState>, Q(p): Q<Params>) -> Result<Response, ApiError> {
    serve(st, parse(p, None)?).await
}

pub async fn by_path(
    st: State<AppState>,
    UrlPath((z, x, y)): UrlPath<(u32, u32, u32)>,
    Q(p): Q<Params>,
) -> Result<Response, ApiError> {
    serve(st, parse(p, Some((z, x, y)))?).await
}

async fn serve(State(st): State<AppState>, q: TileQuery) -> Result<Response, ApiError> {
    let format = q.format;
    let body = blocking(&st.engine, move |e| {
        let t = e.tile(&q)?;
        match q.format {
            Format::Json => to_json_vec(&t),
            Format::Bin => Ok(encode_tile(&t)),
        }
    })
    .await?;
    Ok(match format {
        Format::Json => json_bytes(body),
        Format::Bin => ([(CONTENT_TYPE, HeaderValue::from_static(TILE_MIME))], body).into_response(),
    })
}

impl Engine {
    fn tile(&self, q: &TileQuery) -> atlas_core::Result<TileResponse> {
        let layout =
            self.layout.as_ref().ok_or(atlas_core::Error::NotImplemented("map tiles: no layout loaded (--layout)"))?;
        atlas_layout::tile(self.snap, layout, q.layer, q.z, q.x, q.y, &q.lang)
    }
}

struct Strings<'a> {
    index: HashMap<&'a str, u32>,
    list: Vec<&'a str>,
}

impl<'a> Strings<'a> {
    fn id(&mut self, s: &'a str) -> u32 {
        let next = self.list.len() as u32;
        *self.index.entry(s).or_insert_with(|| {
            self.list.push(s);
            next
        })
    }
}

fn pad(buf: &mut Vec<u8>) {
    buf.resize(buf.len().next_multiple_of(4), 0);
}

fn put_u32s(buf: &mut Vec<u8>, xs: impl IntoIterator<Item = u32>) {
    for x in xs {
        buf.extend_from_slice(&x.to_le_bytes());
    }
}

fn put_f32s(buf: &mut Vec<u8>, xs: impl IntoIterator<Item = f32>) {
    for x in xs {
        buf.extend_from_slice(&x.to_le_bytes());
    }
}

fn kind_code(k: TileNodeKind) -> u8 {
    match k {
        TileNodeKind::Entity => 0,
        TileNodeKind::Cluster => 1,
        TileNodeKind::Concept => 2,
    }
}

fn put_u8s(buf: &mut Vec<u8>, xs: impl IntoIterator<Item = u8>) {
    buf.extend(xs);
    pad(buf);
}

/// Running offsets `0, l0, l0+l1, ...` of a CSR list.
fn offsets(lens: impl Iterator<Item = usize>) -> Vec<u32> {
    let mut acc = 0u32;
    std::iter::once(0).chain(lens.map(|l| {
        acc += l as u32;
        acc
    })).collect()
}

/// Binary tile v1 (module docs).
pub fn encode_tile(t: &TileResponse) -> Vec<u8> {
    let mut s = Strings { index: HashMap::new(), list: Vec::new() };
    let version = s.id(&t.layout_version);
    let items: Vec<&TileNode> = t.nodes.iter().chain(&t.external).collect();
    let item_strs: Vec<[u32; 3]> = items.iter().map(|n| [s.id(&n.id), s.id(&n.label), s.id(&n.community)]).collect();
    let central: Vec<u32> = items.iter().flat_map(|n| n.central.iter().flatten()).map(|c| s.id(c)).collect();
    let local: HashMap<&str, u32> = items.iter().enumerate().map(|(i, n)| (n.id.as_str(), i as u32)).collect();
    let at = |id: &str| local.get(id).copied().unwrap_or(NONE);
    let n_agg_rel: usize = t.aggregates.iter().map(|a| a.hidden_relations.len()).sum();

    let mut b = Vec::with_capacity(96 + items.len() * 34 + t.edges.len() * 14);
    b.extend_from_slice(MAGIC);
    put_u32s(&mut b, [t.z, t.x, t.y, t.max_zoom]);
    put_f32s(&mut b, t.bbox);
    put_u32s(&mut b, [t.layer.code() as u32, version]);
    let counts = [s.list.len(), t.nodes.len(), t.external.len(), t.edges.len(), t.aggregates.len(), n_agg_rel, t.hidden_edges.len(), central.len()];
    put_u32s(&mut b, counts.map(|n| n as u32));

    put_u32s(&mut b, offsets(s.list.iter().map(|x| x.len())));
    for x in &s.list {
        b.extend_from_slice(x.as_bytes());
    }
    pad(&mut b);

    put_f32s(&mut b, items.iter().map(|n| n.x));
    put_f32s(&mut b, items.iter().map(|n| n.y));
    put_u32s(&mut b, items.iter().map(|n| n.size));
    for k in 0..3 {
        put_u32s(&mut b, item_strs.iter().map(|ids| ids[k]));
    }
    put_u32s(&mut b, items.iter().map(|n| n.priority.unwrap_or(NONE)));
    put_u32s(&mut b, offsets(items.iter().map(|n| n.central.as_ref().map_or(0, Vec::len))));
    put_u32s(&mut b, central.iter().copied());
    b.extend(items.iter().map(|n| kind_code(n.kind)));
    put_u8s(&mut b, items.iter().map(|n| n.entity_type.map_or(255, EntityType::code)));

    put_u32s(&mut b, t.edges.iter().map(|e| at(&e.source)));
    put_u32s(&mut b, t.edges.iter().map(|e| at(&e.target)));
    put_u32s(&mut b, t.edges.iter().map(|e| e.count));
    b.extend(t.edges.iter().map(|e| e.relation.code()));
    put_u8s(&mut b, t.edges.iter().map(|e| e.evidence_state.code()));

    put_u32s(&mut b, t.aggregates.iter().map(|a| at(&a.cluster_id)));
    put_u32s(&mut b, t.aggregates.iter().map(|a| a.members));
    put_u32s(&mut b, t.aggregates.iter().map(|a| a.hidden_members));
    put_u32s(&mut b, offsets(t.aggregates.iter().map(|a| a.hidden_relations.len())));
    put_u32s(&mut b, t.aggregates.iter().flat_map(|a| a.hidden_relations.values().copied()));
    put_u8s(&mut b, t.aggregates.iter().flat_map(|a| a.hidden_relations.keys().map(|r| r.code())));

    put_u32s(&mut b, t.hidden_edges.values().copied());
    put_u8s(&mut b, t.hidden_edges.keys().map(|r| r.code()));
    b
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocateParams {
    id: String,
    /// Client cache-buster (layout version); only part of the URI / ETag.
    #[allow(dead_code)]
    v: Option<String>,
}

/// `GET /api/map/locate?id=`: where an entity sits in the layout, for region focus
/// (`{id, x, y, communities: ["L0:3", …]}`, coarse to fine).
pub async fn locate(State(st): State<AppState>, Q(p): Q<LocateParams>) -> Result<Response, ApiError> {
    crate::http::check_id(&p.id)?;
    let e = &st.engine;
    let layout = e.layout.as_ref().ok_or(atlas_core::Error::NotImplemented("map locate: no layout loaded (--layout)"))?;
    let n = e.snap.resolve(&p.id).ok_or_else(|| ApiError::not_found(format!("entity {}", p.id)))?;
    let [x, y] = *layout.positions.get(n as usize).ok_or_else(|| ApiError::not_found(format!("{} not in layout", p.id)))?;
    let communities: Vec<String> = layout
        .levels
        .iter()
        .enumerate()
        .filter_map(|(l, lv)| lv.get(n as usize).map(|c| format!("L{l}:{c}")))
        .collect();
    let body = serde_json::json!({
        "id": e.snap.node_id(n), "x": x, "y": y, "communities": communities, "layout_version": layout.version,
    });
    Ok(axum::Json(body).into_response())
}
