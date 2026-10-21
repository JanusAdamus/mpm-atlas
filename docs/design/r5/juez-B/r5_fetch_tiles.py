"""R5 diagnosis, stage 0: general layer = every node of the structure tiles z0-z2 of a running server (GET only).

    python -I r5_fetch_tiles.py [http://127.0.0.1:8765]   -> r5-general.json next to this file
"""
import http.client, json, sys, urllib.parse
from pathlib import Path

base = urllib.parse.urlsplit(sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8765")
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
nodes, edges, versions = {}, {}, set()
for z in range(3):
    for x in range(2 ** z):
        for y in range(2 ** z):
            conn.request("GET", f"/api/map/tiles?layer=structure&z={z}&x={x}&y={y}&format=json")
            t = json.loads(conn.getresponse().read())
            versions.add(t["layout_version"])
            nodes.update({n["id"]: n for n in t["nodes"]})
            edges.update({(e["source"], e["target"], e["relation"], e["evidence_state"]): e for e in t["edges"]})
assert len(versions) == 1, versions
print(versions, len(nodes), "nodes", len(edges), "edges")
json.dump({"layout_version": versions.pop(), "nodes": list(nodes.values()), "edges": list(edges.values())},
          open(Path(__file__).parent / "r5-general.json", "w", encoding="utf-8"), ensure_ascii=False)
