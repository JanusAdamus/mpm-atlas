//! Probe: cold layout timings on a snapshot (no tiles).  ATLAS_FORCE_TRACE=1 for per-tree timings.
use atlas_core::Snapshot;
use atlas_layout::{ComputeOptions, LayoutParams, compute_with};
fn main() {
    let t = std::time::Instant::now();
    let a: Vec<String> = std::env::args().collect();
    let snap = Snapshot::open(&a[1]).unwrap();
    eprintln!("open {:.3} s", t.elapsed().as_secs_f64());
    let l = compute_with(
        &snap,
        &LayoutParams::default(),
        &ComputeOptions {
            skip_tiles: true,
            ..Default::default()
        },
    )
    .unwrap();
    eprintln!("computed {:.3} s", t.elapsed().as_secs_f64());
    eprintln!("{}", serde_json::to_string(&l.stats).unwrap());
    drop(l);
    eprintln!("dropped {:.3} s", t.elapsed().as_secs_f64());
}
