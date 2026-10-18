//! Probe: Leiden phase timings on a snapshot graph (ATLAS_LEIDEN_TRACE=1 for phases).
//! probe_leiden <snapshot dir> <resolution> [iterations]
use atlas_core::Snapshot;
use atlas_layout::graph::Graph;
use atlas_layout::leiden::{self, LeidenParams};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = std::time::Instant::now();
    let snap = Snapshot::open(&a[1]).unwrap();
    let res: f64 = a[2].parse().unwrap();
    let iterations: u32 = a.get(3).map_or(2, |x| x.parse().unwrap());
    let n = snap.num_nodes() as usize;
    let g = Graph::from_snapshot(&snap, u32::MAX, &vec![true; n]);
    eprintln!(
        "open + graph {:.2} s ({} nodes, {} edges)",
        t.elapsed().as_secs_f64(),
        g.n(),
        g.num_edges()
    );
    let t = std::time::Instant::now();
    let p = leiden::leiden(
        &g,
        &LeidenParams {
            resolution: res,
            iterations,
            ..Default::default()
        },
        None,
    );
    let secs = t.elapsed().as_secs_f64();
    let s = leiden::scores(&g, &p.membership, p.count, leiden::Quality::Modularity, res);
    eprintln!(
        "res {res} x{iterations}: {} communities in {secs:.2} s, Q={:.4} obj={:.4}",
        p.count, s.modularity, s.objective
    );
}
