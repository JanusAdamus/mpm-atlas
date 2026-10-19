"""R5 diagnosis: the 20 seeded connections (seed 20261019) between general-layer concepts.

10 from registry claims with >= 2 distinct concept-type endpoints in the layer (r5_edges.py `pair_claims`),
10 from the aggregated concept -> concept edges of /api/concept (edge_limit=200, both ends concept-type in the layer).
"""
import http.client, json, pickle, random
from pathlib import Path

HERE = Path(__file__).parent
SEED = 20261019
E = pickle.load(open(HERE / "r5-load.pkl", "rb"))["ents"]
x = pickle.load(open(HERE / "r5-edges.pkl", "rb"))
conc = {e for e in E if E[e][0]["type"] == "concept"}
pool = sorted(p for p in x["pair_claims"] if all(e in conc for e in p[3]))
print("direct pool", len(pool))
for p in random.Random(SEED).sample(pool, 10):
    print("D", p)
conn = http.client.HTTPConnection("127.0.0.1", 8765, timeout=60)
apool = []
for e in sorted(x["agg"]):
    conn.request("GET", f"/api/concept?id={e}&limit=1&edge_limit=200&lang=es")
    j = json.loads(conn.getresponse().read())
    apool += [(e, ed["concept"]["id"], ed["total"], tuple((s["relation"], s["evidence_state"], s["count"], s["example_claim"])
              for s in ed["stats"])) for ed in j["edges_out"] if ed["concept"]["id"] in conc]
print("aggregate pool", len(apool))
for a in random.Random(SEED).sample(apool, 10):
    print("A", a)
