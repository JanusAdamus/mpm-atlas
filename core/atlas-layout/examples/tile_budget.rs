//! Tile budgets on a real snapshot: build the layout warm-started from `--prior`
//! (read only) with the given budgets, then report the JSON size of the 85 structure
//! tiles z0-z3 that AtlasBench requests, and how many nodes the general zooms draw.
//! Nothing is written unless `--out` is given.
//!
//! cargo run --release -p atlas-layout --example tile_budget -- --snapshot data/snapshot \
//!     --prior data/layout --general-tile-nodes 600 --general-edges 512 --edges 1024

use atlas_core::{Projection, Snapshot};
use atlas_layout::{ComputeOptions, LayoutParams, Prior, compute_with, tile};
use serde_json::json;
use std::collections::HashSet;
use std::path::PathBuf;

fn main() {
    let v: Vec<String> = std::env::args().skip(1).collect();
    let (mut snapshot, mut prior, mut out) = (PathBuf::from("data/snapshot"), None, None);
    let mut p = LayoutParams::default();
    for kv in v.chunks(2) {
        let val = kv.get(1).expect("flag needs a value");
        match kv[0].as_str() {
            "--snapshot" => snapshot = val.into(),
            "--prior" => prior = Some(PathBuf::from(val)),
            "--out" => out = Some(PathBuf::from(val)),
            "--general-tile-nodes" => p.tiles.general_tile_nodes = val.parse().unwrap(),
            "--general-edges" => p.tiles.general_edges = val.parse().unwrap(),
            "--edges" => p.tiles.edges = val.parse().unwrap(),
            "--entities" => p.tiles.entities = val.parse().unwrap(),
            f => panic!("unknown flag {f}"),
        }
    }
    let snap = Snapshot::open(&snapshot).unwrap();
    let prior = prior.and_then(|d| Prior::open(&d).unwrap());
    let l = compute_with(&snap, &p, &ComputeOptions { prior, ..Default::default() }).unwrap();
    let mut sizes = Vec::new();
    let mut general: HashSet<String> = HashSet::new();
    for z in 0..4u32 {
        for x in 0..1u32 << z {
            for y in 0..1u32 << z {
                let t = tile(&snap, &l, Projection::Structure, z, x, y, "es").unwrap();
                if z <= p.tiles.general_zoom {
                    general.extend(t.nodes.iter().map(|n| n.id.clone()));
                }
                sizes.push((serde_json::to_vec(&t).unwrap().len(), format!("{z}/{x}/{y}"), t.nodes.len(), t.edges.len()));
            }
        }
    }
    sizes.sort_by(|a, b| b.cmp(a));
    // In-process cost of the AtlasBench mix (tile + JSON), no HTTP: 20 passes over the 85 tiles.
    let mut ms: Vec<f64> = Vec::new();
    for _ in 0..20 {
        for z in 0..4u32 {
            for x in 0..1u32 << z {
                for y in 0..1u32 << z {
                    let t0 = std::time::Instant::now();
                    let t = tile(&snap, &l, Projection::Structure, z, x, y, "es").unwrap();
                    std::hint::black_box(serde_json::to_vec(&t).unwrap());
                    ms.push(t0.elapsed().as_secs_f64() * 1e3);
                }
            }
        }
    }
    ms.sort_by(f64::total_cmp);
    let q = |p: f64| ms[((ms.len() - 1) as f64 * p).round() as usize];
    let report = json!({
        "budget": p.tiles, "layout_version": l.version,
        "general_nodes_z0_z2": general.len(),
        "inprocess_tile_json_ms": {"n": ms.len(), "p50": q(0.5), "p95": q(0.95), "max": q(1.0)},
        "json_bytes_p95": sizes[sizes.len() / 20].0,
        "z0_json_bytes": sizes.iter().find(|s| s.1 == "0/0/0").map(|s| s.0),
        "heaviest": sizes.iter().take(8).map(|s| json!({"tile": s.1, "json_bytes": s.0, "nodes": s.2, "edges": s.3})).collect::<Vec<_>>(),
        "zooms": l.tiles.meta.zooms.iter().take(4).map(|z| json!({"z": z.z, "visible": z.visible_entities, "unplaced": z.unplaced, "bytes_max": z.bytes_max})).collect::<Vec<_>>(),
        "written": out.as_ref().map(|d| l.write(d).unwrap().version),
    });
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
