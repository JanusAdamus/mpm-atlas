"""Juez B: aggregated concept edges shown as 'extracted' -- are they stronger than their weakest link?

    python -I b_weakest.py [http://127.0.0.1:8823]  -> b-weakest.json next to this file

For each aggregate stat with evidence_state 'extracted' (out-edges of general-layer concepts),
take its example_claim and look up, in the registry, the membership states of the two
declarations in the two concepts.  Stronger than weakest link = some needed membership is
model_proposed (or weaker) in every current version.
"""
import http.client, json, pickle, sqlite3, sys, urllib.parse
from pathlib import Path

HERE = Path(__file__).parent
base = urllib.parse.urlsplit(sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8823")
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
RANK = {"formally_linked": 4, "editorially_reviewed": 3, "extracted": 2, "model_proposed": 1, "disputed": 0}
x = pickle.load(open(HERE / "r5-edges.pkl", "rb"))


def memb_states(decl: str, concept: str) -> list:
    return [r[0] for r in c.execute("""select cl.evidence_state from claims cl
        join claim_participants a on a.claim_id=cl.id and a.entity_id=?
        join claim_participants b on b.claim_id=cl.id and b.entity_id=?
        where cl.relation='member_of_concept'""", (decl, concept))]


rows = []
for e, v in sorted(x["agg"].items()):
    if not v or not any(s[2] == "extracted" and s[0] == "edges_out" for s in v["stats"]):
        continue
    conn.request("GET", f"/api/concept?id={urllib.parse.quote(e)}&limit=1&edge_limit=200&lang=es")
    j = json.loads(conn.getresponse().read())
    for ed in j["edges_out"]:
        for s in ed["stats"]:
            if s["evidence_state"] != "extracted":
                continue
            parts = c.execute("select role, entity_id from claim_participants where claim_id=? order by position",
                              (s["example_claim"],)).fetchall()
            subj = [p[1] for p in parts if p[0] == "subject"]
            objs = [p[1] for p in parts if p[0] != "subject"]
            src = {d: memb_states(d, e) for d in subj}
            dst = {d: memb_states(d, ed["concept"]["id"]) for d in objs}
            best_src = max((RANK.get(st, -1) for l in src.values() for st in l), default=-1)
            best_dst = max((RANK.get(st, -1) for l in dst.values() for st in l), default=-1)
            rows.append({"from": e, "to": ed["concept"]["id"], "count": s["count"], "candidate": ed["candidate"],
                         "example_claim": s["example_claim"], "src_memberships": src, "dst_memberships": dst,
                         "stronger_than_weakest": min(best_src, best_dst) < RANK["extracted"]})
out = {"extracted_aggregate_stats": len(rows), "candidate_false": sum(1 for r in rows if not r["candidate"]),
       "stronger_than_weakest_link": sum(1 for r in rows if r["stronger_than_weakest"]), "rows": rows}
json.dump(out, open(HERE / "b-weakest.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print({k: v for k, v in out.items() if k != "rows"})
