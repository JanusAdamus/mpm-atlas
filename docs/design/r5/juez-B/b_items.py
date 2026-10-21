"""Juez B: what a reader sees for each of the 20 sampled connections (read-only).

    python -I b_items.py [http://127.0.0.1:8811]  -> b-items.json next to this file

Per item: labels/descriptions ES+EN of both ends (registry), the relation verb and its
chain_authorizes text as the UI words them, the shown state, the candidate mark, and the
witness claim with its provenance.  For aggregated edges, the witness declarations and the
two memberships that carry the edge.
"""
import http.client, json, sqlite3, sys, urllib.parse
from pathlib import Path

HERE = Path(__file__).parent
base = urllib.parse.urlsplit(sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8811")
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)


def ent(eid: str) -> dict:
    row = c.execute("select data, merged_into from entities where id=?", (eid,)).fetchone()
    if not row:
        return {"id": eid, "missing": True}
    d = json.loads(row[0])
    keep = {k: d.get(k) for k in ("type", "labels", "descriptions", "aliases")}
    for k in ("statements", "lean_name", "docstring", "kind"):
        if d.get(k):
            keep[k] = d[k] if not isinstance(d[k], str) else d[k][:300]
    return {"id": eid, "merged_into": row[1], **keep}


def claim(cid: str) -> dict:
    cl = c.execute("select id, relation, evidence_state from claims where id=?", (cid,)).fetchone()
    parts = c.execute("select role, entity_id, position from claim_participants where claim_id=? order by position", (cid,)).fetchall()
    prov = c.execute("select * from provenance where claim_id=?", (cid,)).fetchall()
    cols = [r[1] for r in c.execute("pragma table_info(provenance)")]
    return {"claim": cl, "participants": parts, "provenance": [dict(zip(cols, p)) for p in prov]}


def membership(member: str, concept: str) -> list:
    rows = c.execute("""select cl.id, cl.evidence_state from claims cl
        join claim_participants a on a.claim_id=cl.id and a.entity_id=?
        join claim_participants b on b.claim_id=cl.id and b.entity_id=?
        where cl.relation='member_of_concept'""", (member, concept)).fetchall()
    return [{"claim": r[0], "state": r[1], **{k: v for k, v in claim(r[0]).items() if k == "provenance"}} for r in rows]


sample = json.load(open(HERE / "r5-protocol-sample.json", encoding="utf-8"))
out = []
for it in sample["items"]:
    rec = dict(it)
    if it["n"].startswith("D"):
        a, b = it["ends"]
        rec["ends_detail"] = [ent(a), ent(b)]
        rec["witness"] = claim(it["claim"])
    else:
        a, b = it["from"], it["to"]
        rec["ends_detail"] = [ent(a), ent(b)]
        conn.request("GET", f"/api/concept?id={urllib.parse.quote(a)}&limit=1&edge_limit=200&lang=es")
        j = json.loads(conn.getresponse().read())
        edge = next(e for e in j["edges_out"] if e["concept"]["id"] == b)
        rec["api_edge"] = edge
        ex = claim(edge["stats"][0]["example_claim"])
        rec["witness"] = ex
        decl = [p[1] for p in ex["participants"]]
        rec["witness_decls"] = [ent(d) for d in decl]
        rec["witness_memberships"] = {d: [membership(d, a), membership(d, b)] for d in decl}
    out.append(rec)
json.dump(out, open(HERE / "b-items.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print("items", len(out))
