"""R5 protocol (docs/plans/r5/protocolo.md): the NEW sample of 20 general-layer connections.

Same design as r5_sample20.py (10 direct registry claims + 10 aggregated concept edges, both ends
concept-type in the general layer) with a new seed.  Run on the round's FINAL snapshot, after the
chain r5_fetch_tiles.py -> r5_load.py -> r5_edges.py has been re-run against that server:

    python -I r5_protocol_sample.py [http://127.0.0.1:8765]  -> r5-protocol-sample.json next to this file

The draw is a pure function of the seed and the pools, so it is reproducible from the snapshot hash
that r5_fetch_tiles.py records (layout_version) and the registry's event_seq.
"""
import http.client, json, pickle, random, sys, urllib.parse
from pathlib import Path

HERE = Path(__file__).parent
SEED = 20261021  # juez B ronda 2: semilla fijada por el orquestador para los dos jueces (directo 20261021, agregado 20261022)
base = urllib.parse.urlsplit(sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8765")
E = pickle.load(open(HERE / "r5-load.pkl", "rb"))["ents"]
x = pickle.load(open(HERE / "r5-edges.pkl", "rb"))
conc = {e for e in E if E[e][0]["type"] == "concept"}
pool = sorted(p for p in x["pair_claims"] if all(e in conc for e in p[3]))
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
apool = []
for e in sorted(x["agg"]):
    conn.request("GET", f"/api/concept?id={e}&limit=1&edge_limit=200&lang=es")
    j = json.loads(conn.getresponse().read())
    apool += [(e, ed["concept"]["id"], ed["total"], tuple((s["relation"], s["evidence_state"], s["count"], s["example_claim"])
              for s in ed["stats"])) for ed in j["edges_out"] if ed["concept"]["id"] in conc]
direct = random.Random(SEED).sample(pool, 10)
agg = random.Random(SEED + 1).sample(apool, 10)
out = {"seed": SEED, "direct_pool": len(pool), "aggregate_pool": len(apool),
       "items": [{"n": f"D{i + 1}", "claim": p[0], "relation": p[1], "state": p[2], "ends": p[3]} for i, p in enumerate(direct)]
       + [{"n": f"A{i + 1}", "from": a[0], "to": a[1], "total": a[2], "stats": a[3]} for i, a in enumerate(agg)]}
json.dump(out, open(HERE / "r5-protocol-sample.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print("pools", len(pool), len(apool), "-> r5-protocol-sample.json")
