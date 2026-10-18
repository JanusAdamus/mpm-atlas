//! `GET /api/coverage` (`Store.coverage()` of the snapshot, computed once at start)
//! and `GET /api/health` (what is loaded and which lane serves each endpoint).

use atlas_core::Snapshot;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;

/// `Store.coverage()` over the snapshot.  Same keys and meaning: claims in any state,
/// canonical entities only, entities without claims are "unknown".  Extra keys say
/// which snapshot answered (`content_hash`, `source_event_seq`).
pub fn compute(snap: &Snapshot, generated_at: &str) -> Value {
    let st = snap.stats();
    let n_sources = snap.num_sources() as usize;
    let by_source = (0..snap.num_claims())
        .into_par_iter()
        .fold(
            || vec![0u64; n_sources],
            |mut acc, c| {
                let mut seen: Vec<u32> = snap.claim_provenance(c).map(|p| p.source).collect();
                seen.sort_unstable();
                seen.dedup();
                for s in seen {
                    acc[s as usize] += 1;
                }
                acc
            },
        )
        .reduce(|| vec![0u64; n_sources], |a, b| a.iter().zip(&b).map(|(x, y)| x + y).collect());
    let claims_by_source: BTreeMap<&str, u64> = by_source
        .iter()
        .enumerate()
        .filter(|(_, n)| **n > 0)
        .map(|(s, n)| (snap.source_id(s as u32), *n))
        .collect();
    json!({
        "generated_at": generated_at,
        "schema_version": st.schema_version,
        "sources": st.sources,
        "entities_by_type": st.nodes_by_type,
        "claims_by_relation": st.claims_by_relation,
        "claims_by_state": st.claims_by_state,
        "unknown_by_type": st.unknown_by_type,
        "claims_by_source": claims_by_source,
        "active_merges": st.merged_nodes,
        "content_hash": st.content_hash,
        "source_event_seq": st.source_event_seq,
    })
}

pub async fn coverage(State(st): State<AppState>) -> Response {
    axum::Json(st.engine.coverage.clone()).into_response()
}

pub async fn health(State(st): State<AppState>) -> Response {
    let e = &st.engine;
    let snap = e.snap;
    axum::Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "started_at": e.started_at,
        "snapshot": {
            "path": snap.path().display().to_string(),
            "content_hash": snap.content_hash_hex(),
            "source_event_seq": snap.header().source_event_seq,
            "nodes": snap.num_nodes(),
            "claims": snap.num_claims(),
            "edges": snap.num_edges(),
        },
        "layout_version": e.layout.as_ref().map(|l| l.version.as_str()),
        // Zoom semantics of the map: z0..=general_zoom show concepts and areas only;
        // declarations appear from detail_zoom (null without a concept layer).
        "map": e.layout.as_ref().map(|l| {
            let m = &l.tiles.meta;
            json!({
                "max_zoom": m.zmax, "general_zoom": m.general_zoom,
                "detail_zoom": m.general_zoom.map(|_| m.budget.detail_zoom), "general_nodes": m.general_nodes,
            })
        }),
        "concepts": snap.concepts().len(),
        "components": e.components,
    }))
    .into_response()
}

/// ISO-8601 UTC timestamp of now (civil-from-days, no time crate needed).
pub fn now_utc() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn now_utc_is_iso8601() {
        let s = super::now_utc();
        assert_eq!(s.len(), 20, "{s}");
        assert!(s.starts_with("20") && s.ends_with('Z'), "{s}");
    }
}
