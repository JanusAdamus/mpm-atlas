//! Closed-loop HTTP/1.1 load generator for `atlas serve` (keep-alive, N connections).
//!
//!   loadgen urls --snapshot DIR --out DIR [--n 2000] [--seed 7]
//!       writes one URL list per endpoint, sampled uniformly from the snapshot's nodes
//!   loadgen run --base http://127.0.0.1:8765 --urls FILE [--conns 64] [--secs 10] [--encoding br]
//!       prints one JSON line: requests, req/s, p50/p95/p99/max latency, status counts, bytes
//!
//! Each connection loops over the list from its own offset, so the same mix is
//! replayed; latency is measured from send to the last body byte.

use atlas_core::{Dir, Snapshot};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn num(args: &[String], name: &str, default: u64) -> u64 {
    arg(args, name).map_or(default, |v| v.parse().expect("number"))
}

/// splitmix64: deterministic sampling without a rand dependency.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b':' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn urls(args: &[String]) {
    let snap = Snapshot::open(arg(args, "--snapshot").expect("--snapshot")).expect("open snapshot");
    let out = PathBuf::from(arg(args, "--out").expect("--out"));
    std::fs::create_dir_all(&out).unwrap();
    let n = num(args, "--n", 2000) as usize;
    let mut rng = num(args, "--seed", 7);
    let nodes = snap.num_nodes() as u64;
    let pick = |rng: &mut u64| loop {
        let v = (next(rng) % nodes) as u32;
        if !snap.is_merged(v) {
            return v;
        }
    };
    let sample: Vec<u32> = (0..n).map(|_| pick(&mut rng)).collect();
    let pairs: Vec<(u32, u32)> = (0..n).map(|_| (pick(&mut rng), pick(&mut rng))).collect();
    let (out_csr, in_csr) = (snap.csr(Dir::Out), snap.csr(Dir::In));
    let degrees: Vec<usize> = sample.iter().map(|&v| out_csr.degree(v) + in_csr.degree(v)).collect();
    let words: Vec<String> = sample
        .iter()
        .map(|&v| {
            let label = snap.label(v, "es");
            let first = label.split_whitespace().next().unwrap_or(label);
            first.chars().take(4).collect()
        })
        .collect();
    let write = |name: &str, lines: Vec<String>| std::fs::write(out.join(name), lines.join("\n")).unwrap();
    let id = |v: u32| enc(snap.node_id(v));
    write("health.txt", vec!["/api/health".into()]);
    write("coverage.txt", vec!["/api/coverage".into()]);
    write("entity.txt", sample.iter().map(|&v| format!("/api/entity/{}?lang=es", id(v))).collect());
    write("neighborhood_d1.txt", sample.iter().map(|&v| format!("/api/neighborhood?id={}&depth=1", id(v))).collect());
    write(
        "neighborhood_d2.txt",
        sample.iter().map(|&v| format!("/api/neighborhood?id={}&depth=2&limit=500", id(v))).collect(),
    );
    write("search.txt", words.iter().map(|w| format!("/api/search?q={}&lang=es", enc(w))).collect());
    write("path.txt", pairs.iter().map(|&(a, b)| format!("/api/path?from={}&to={}", id(a), id(b))).collect());
    // Random pairs almost never connect (they measure abstention).  These do: a
    // random walk of 2-3 outgoing edges of one relation from each sampled node.
    let mut near = Vec::new();
    for &v in &sample {
        let first = out_csr.edges(v);
        if first.is_empty() {
            continue;
        }
        let rel = first.get((next(&mut rng) % first.len() as u64) as usize).relation;
        let (mut at, mut hops) = (v, 0);
        while hops < 2 + (next(&mut rng) % 2) as usize {
            let step: Vec<u32> = out_csr.edges(at).iter().filter(|e| e.relation == rel && e.nbr != v).map(|e| e.nbr).collect();
            if step.is_empty() {
                break;
            }
            at = step[(next(&mut rng) % step.len() as u64) as usize];
            hops += 1;
        }
        if hops >= 2 {
            near.push(format!("/api/path?from={}&to={}", id(v), id(at)));
        }
    }
    eprintln!("path_near: {} of {n} samples have a 2-3 hop single-relation walk", near.len());
    write("path_near.txt", near);
    let mut tiles = vec![];
    for z in 0..=6u32 {
        for x in 0..(1u32 << z) {
            for y in 0..(1u32 << z) {
                tiles.push(format!("/api/map/tiles/{z}/{x}/{y}?format=bin"));
            }
        }
    }
    let k = tiles.len();
    write("tiles_bin.txt", tiles.iter().take(k).cloned().collect());
    write("tiles_json.txt", tiles.iter().map(|t| t.replace("format=bin", "format=json")).collect());
    write("locate.txt", sample.iter().map(|&v| format!("/api/map/locate?id={}", id(v))).collect());
    let mut d = degrees.clone();
    d.sort_unstable();
    eprintln!(
        "wrote {n} samples; degree p50 {} p95 {} max {}",
        d[d.len() / 2],
        d[d.len() * 95 / 100],
        d[d.len() - 1]
    );
}

fn pct(sorted: &[u32], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize] as f64 / 1000.0
}

/// One keep-alive HTTP/1.1 exchange on a blocking socket: returns (status, body bytes).
/// Handles `Content-Length` and `Transfer-Encoding: chunked` (compressed responses stream).
fn exchange(stream: &mut BufReader<TcpStream>, req: &[u8]) -> std::io::Result<(u16, u64)> {
    stream.get_mut().write_all(req)?;
    let mut line = String::new();
    stream.read_line(&mut line)?;
    let status: u16 = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let (mut len, mut chunked) = (0usize, false);
    loop {
        line.clear();
        stream.read_line(&mut line)?;
        let l = line.trim_end();
        if l.is_empty() {
            break;
        }
        let lower = l.to_ascii_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            len = v.trim().parse().unwrap_or(0);
        } else if lower.starts_with("transfer-encoding:") && lower.contains("chunked") {
            chunked = true;
        }
    }
    if !chunked {
        std::io::copy(&mut stream.by_ref().take(len as u64), &mut std::io::sink())?;
        return Ok((status, len as u64));
    }
    let mut total = 0u64;
    loop {
        line.clear();
        stream.read_line(&mut line)?;
        let n = usize::from_str_radix(line.trim().split(';').next().unwrap_or("0"), 16).unwrap_or(0);
        std::io::copy(&mut stream.by_ref().take(n as u64 + 2), &mut std::io::sink())?; // data + CRLF
        total += n as u64;
        if n == 0 {
            return Ok((status, total));
        }
    }
}

/// Closed loop: each connection is one OS thread with a blocking keep-alive socket
/// (TCP_NODELAY).  ponytail: thread per connection caps out around a few thousand
/// connections; an async client would be needed past that.  The first version used
/// hyper-util's pooled client, which added ~2.5 ms per request on this machine
/// (1 connection: p50 2.9 ms vs 0.38 ms from a plain socket against the same server).
fn run(args: Vec<String>) {
    let base = arg(&args, "--base").unwrap_or_else(|| "http://127.0.0.1:8765".into());
    let host = base.trim_start_matches("http://").trim_end_matches('/').to_string();
    let encoding = arg(&args, "--encoding");
    let list: Vec<Vec<u8>> = std::fs::read_to_string(arg(&args, "--urls").expect("--urls"))
        .expect("read urls")
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let ae = encoding.as_ref().map_or(String::new(), |e| format!("accept-encoding: {e}\r\n"));
            format!("GET {l} HTTP/1.1\r\nhost: {host}\r\n{ae}\r\n").into_bytes()
        })
        .collect();
    let list = std::sync::Arc::new(list);
    let conns = num(&args, "--conns", 64) as usize;
    let secs = num(&args, "--secs", 10);
    let start = Instant::now();
    let deadline = start + Duration::from_secs(secs);
    let workers: Vec<_> = (0..conns)
        .map(|w| {
            let (list, host) = (list.clone(), host.clone());
            std::thread::spawn(move || {
                let tcp = TcpStream::connect(&host).expect("connect");
                tcp.set_nodelay(true).unwrap();
                let mut stream = BufReader::with_capacity(1 << 16, tcp);
                let (mut lat, mut status, mut bytes) = (Vec::new(), BTreeMap::<u16, u64>::new(), 0u64);
                let mut i = w * list.len() / conns.max(1);
                while Instant::now() < deadline {
                    let t = Instant::now();
                    let (code, n) = exchange(&mut stream, &list[i % list.len()]).expect("request");
                    lat.push(t.elapsed().as_micros() as u32);
                    bytes += n;
                    *status.entry(code).or_default() += 1;
                    i += 1;
                }
                (lat, status, bytes)
            })
        })
        .collect();
    let mut lat = Vec::new();
    let mut status: BTreeMap<u16, u64> = BTreeMap::new();
    let mut bytes = 0;
    for t in workers {
        let (l, s, b) = t.join().unwrap();
        lat.extend(l);
        for (k, v) in s {
            *status.entry(k).or_default() += v;
        }
        bytes += b;
    }
    let elapsed = start.elapsed().as_secs_f64();
    lat.sort_unstable();
    let out = serde_json::json!({
        "urls": arg(&args, "--urls"), "distinct_urls": list.len(), "conns": conns, "secs": elapsed,
        "encoding": encoding, "requests": lat.len(), "req_per_s": lat.len() as f64 / elapsed,
        "p50_ms": pct(&lat, 0.50), "p95_ms": pct(&lat, 0.95), "p99_ms": pct(&lat, 0.99), "max_ms": pct(&lat, 1.0),
        "status": status, "mean_body_bytes": bytes as f64 / lat.len().max(1) as f64,
    });
    println!("{out}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("urls") => urls(&args),
        Some("run") => run(args),
        _ => eprintln!("usage: loadgen urls|run (see source header)"),
    }
}
