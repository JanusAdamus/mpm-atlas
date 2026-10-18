// Closed-loop HTTP/1.1 load generator for `atlas serve` (keep-alive, raw sockets, TCP_NODELAY).
//
//   node bench/load.mjs --urls FILE [--base http://127.0.0.1:8765] [--conns 64] [--secs 10] [--workers 8] [--encoding br]
//
// URL lists come from `loadgen urls` (core/atlas-server/examples/loadgen.rs), which samples
// entities from the snapshot.  Prints one JSON line like `loadgen run`.
//
// Why a Node client next to the Rust one: on this machine every request from the Rust
// client binary costs ~2.5 ms even against a trivial Node server (1 connection: Rust p50
// 2.5 ms, Node p50 0.26 ms, Python http.client 0.34 ms, same requests), so the Rust
// client measured itself, not the server.  Each worker thread owns conns/workers sockets.
import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import { readFileSync } from "node:fs";
import net from "node:net";

const CRLF2 = Buffer.from("\r\n\r\n");

/** Length of one complete response at the start of `buf` (Content-Length or chunked), or -1. */
export function responseLength(buf) {
  const head = buf.indexOf(CRLF2);
  if (head < 0) return -1;
  const headers = buf.subarray(0, head).toString("latin1").toLowerCase();
  const cl = /\r\ncontent-length:\s*(\d+)/.exec(headers);
  let pos = head + 4;
  if (cl) return buf.length >= pos + Number(cl[1]) ? pos + Number(cl[1]) : -1;
  if (!/\r\ntransfer-encoding:[^\r]*chunked/.test(headers)) return pos; // e.g. 304, no body
  for (;;) {
    const eol = buf.indexOf("\r\n", pos);
    if (eol < 0) return -1;
    const n = parseInt(buf.subarray(pos, eol).toString("latin1"), 16);
    pos = eol + 2 + n + 2;
    if (buf.length < pos) return -1;
    if (n === 0) return pos;
  }
}

function worker({ host, port, reqs, conns, secs, offset }) {
  const lat = [], status = {};
  let bytes = 0, open = conns;
  const end = performance.now() + secs * 1000;
  for (let c = 0; c < conns; c++) {
    let i = offset + Math.floor((c * reqs.length) / conns), t = 0, buf = Buffer.alloc(0);
    const s = net.connect(port, host);
    s.setNoDelay(true);
    const send = () => {
      if (performance.now() > end) {
        s.destroy();
        if (--open === 0) parentPort.postMessage({ lat, status, bytes });
        return;
      }
      t = performance.now();
      s.write(reqs[i++ % reqs.length]);
    };
    s.on("connect", send);
    s.on("error", (e) => { status[`error:${e.code}`] = (status[`error:${e.code}`] ?? 0) + 1; });
    s.on("data", (d) => {
      buf = buf.length ? Buffer.concat([buf, d]) : d;
      const n = responseLength(buf);
      if (n < 0) return;
      lat.push(performance.now() - t);
      const code = buf.subarray(9, 12).toString("latin1");
      status[code] = (status[code] ?? 0) + 1;
      bytes += n;
      buf = buf.subarray(n);
      send();
    });
  }
}

if (!isMainThread) {
  worker(workerData);
} else if (process.argv[1]?.endsWith("load.mjs")) {
  const arg = (name, def) => {
    const i = process.argv.indexOf(`--${name}`);
    return i > 0 ? process.argv[i + 1] : def;
  };
  const base = new URL(arg("base", "http://127.0.0.1:8765"));
  const encoding = arg("encoding", null);
  const conns = Number(arg("conns", 64)), secs = Number(arg("secs", 10));
  const workers = Math.min(Number(arg("workers", 8)), conns);
  const urls = readFileSync(arg("urls"), "utf8").split("\n").filter(Boolean);
  const ae = encoding ? `accept-encoding: ${encoding}\r\n` : "";
  const reqs = urls.map((u) => `GET ${u} HTTP/1.1\r\nhost: ${base.host}\r\n${ae}\r\n`);
  const t0 = performance.now();
  const parts = await Promise.all(
    Array.from({ length: workers }, (_, w) => {
      const share = Math.floor(conns / workers) + (w < conns % workers ? 1 : 0);
      const data = { host: base.hostname, port: Number(base.port || 80), reqs, conns: share, secs, offset: Math.floor((w * reqs.length) / workers) };
      return new Promise((ok, fail) => new Worker(new URL(import.meta.url), { workerData: data }).once("message", ok).once("error", fail));
    }),
  );
  const elapsed = (performance.now() - t0) / 1000;
  const lat = new Float64Array(parts.flatMap((p) => p.lat)).sort();
  const status = {};
  for (const p of parts) for (const [k, v] of Object.entries(p.status)) status[k] = (status[k] ?? 0) + v;
  const pct = (q) => (lat.length ? +lat[Math.round((lat.length - 1) * q)].toFixed(3) : 0);
  console.log(JSON.stringify({
    urls: arg("urls"), distinct_urls: urls.length, conns, workers, secs: elapsed, encoding, requests: lat.length,
    req_per_s: lat.length / elapsed, p50_ms: pct(0.5), p95_ms: pct(0.95), p99_ms: pct(0.99), max_ms: pct(1),
    status, mean_response_bytes: parts.reduce((a, p) => a + p.bytes, 0) / Math.max(lat.length, 1),
  }));
}
