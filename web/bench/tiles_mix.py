"""AtlasBench tile mix, alone: the 85 structure tiles z0-z3 as JSON, 500 requests at
concurrency 1 over one keep-alive connection, one warm-up pass (bench/__main__.py
`_interaction_load`, same client and same code).  Also prints the z0 size per format.

    python web/bench/tiles_mix.py --http http://127.0.0.1:8769 [--n 500] [--runs 3]
"""

import argparse
import http.client
import json
import sys
import urllib.parse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))  # repo root, for `bench`
from bench.__main__ import _load_test
from bench.backends import HttpBackend
from bench.metrics import p95_ci, percentiles

TILES = [(z, x, y) for z in range(4) for x in range(2 ** z) for y in range(2 ** z)]


def size(base: str, fmt: str, z: int = 0, x: int = 0, y: int = 0) -> int:
    """Uncompressed body bytes (keep-alive client, no Accept-Encoding)."""
    u = urllib.parse.urlsplit(base)
    conn = http.client.HTTPConnection(u.hostname, u.port, timeout=60)
    try:
        conn.request("GET", f"/api/map/tiles?layer=structure&z={z}&x={x}&y={y}&format={fmt}")
        return len(conn.getresponse().read())
    finally:
        conn.close()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--http", required=True)
    ap.add_argument("--n", type=int, default=500)
    ap.add_argument("--runs", type=int, default=3)
    a = ap.parse_args()
    http = HttpBackend(a.http)
    call = lambda t: http.get("/api/map/tiles", {"layer": "structure", "z": t[0], "x": t[1], "y": t[2]})  # noqa: E731
    runs = []
    for _ in range(a.runs):
        ms, errors = _load_test(call, TILES, a.n, 1)
        runs.append({**percentiles(ms), "p95_ci": p95_ci(ms)["ci95"], "errors": errors})
    json_sizes = {t: size(a.http, "json", *t) for t in TILES}
    per_zoom = {z: max(v for t, v in json_sizes.items() if t[0] == z) for z in range(4)}
    heaviest = sorted(json_sizes.items(), key=lambda kv: -kv[1])[:6]
    out = {"base": a.http, "tiles": len(TILES), "runs": runs, "retries": http.retries,
           "z0_bytes": {"bin": size(a.http, "bin"), "json": size(a.http, "json")},
           "max_json_bytes_per_zoom": per_zoom,
           "heaviest_json": [{"tile": "/".join(map(str, t)), "bytes": v} for t, v in heaviest]}
    json.dump(out, sys.stdout, indent=1)
    print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
