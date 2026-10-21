"""Juez A: tarjeta de lectura por ítem de r5-protocol-sample.json (lo que ve el lector + testigo).

    python -I build_cards.py http://127.0.0.1:8794  -> r5-cards.json
"""
import http.client, json, sqlite3, sys, urllib.parse
from pathlib import Path

HERE = Path(__file__).parent
base = urllib.parse.urlsplit(sys.argv[1])
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
db = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
vocab = json.load(open("C:/MPM/shapes/vocab.json", encoding="utf-8"))
rels = {r["id"] if isinstance(r, dict) and "id" in r else k: r for k, r in
        (vocab["relations"].items() if isinstance(vocab["relations"], dict) else [(r["id"], r) for r in vocab["relations"]])}


def get(path):
    conn.request("GET", path)
    return json.loads(conn.getresponse().read())


def ent(eid):
    j = get(f"/api/entity/{urllib.parse.quote(eid, safe=':')}?lang=es&limit=1")
    e = j["entity"]
    return {"id": eid, "labels": e.get("labels"), "descriptions": e.get("descriptions"), "type": e.get("type"),
            "derived": ((j.get("concept") or {}).get("as_concept") or {}).get("derived")}


def claim(cid):
    st, rel = db.execute("select evidence_state, relation from claims where id=?", (cid,)).fetchone()
    parts = db.execute("select role, entity_id from claim_participants where claim_id=? order by position", (cid,)).fetchall()
    prov = db.execute("select source_id, locator, extractor, extractor_version from provenance where claim_id=?", (cid,)).fetchall()
    return {"id": cid, "relation": rel, "state": st, "participants": parts, "provenance": prov}


def membership(decl, concept):
    rows = db.execute("""select cl.id, cl.evidence_state, p.locator, p.extractor from claims cl
        join claim_participants s on s.claim_id=cl.id and s.role='subject' and s.entity_id=?
        join claim_participants o on o.claim_id=cl.id and o.role='object' and o.entity_id=?
        left join provenance p on p.claim_id=cl.id
        where cl.relation='member_of_concept' and cl.evidence_state not in ('superseded','retracted')""", (decl, concept)).fetchall()
    return rows


sample = json.load(open(HERE / "r5-protocol-sample.json", encoding="utf-8"))
cards = []
for it in sample["items"]:
    if it["n"].startswith("D"):
        a, b = it["ends"]
        c = claim(it["claim"])
        card = {"n": it["n"], "from": ent(a), "to": ent(b), "relation": it["relation"], "state_shown": it["state"],
                "candidate": False, "witness": c}
    else:
        j = get(f"/api/concept?id={urllib.parse.quote(it['from'], safe=':')}&limit=1&edge_limit=200&lang=es")
        ed = next(e for e in j["edges_out"] if e["concept"]["id"] == it["to"])
        w = claim(ed["stats"][0]["example_claim"])
        es = [e for r, e in w["participants"]]
        d1 = es[0]
        d2 = next((e for e in es[1:] if membership(e, it["to"])), es[1])
        card = {"n": it["n"], "from": ent(it["from"]), "to": ent(it["to"]), "relation": ed["stats"][0]["relation"],
                "state_shown": ed["stats"][0]["evidence_state"], "candidate": ed["candidate"], "total": ed["total"],
                "witness": w, "witness_decls": [ent(d1), ent(d2)],
                "memberships": [membership(d1, it["from"]), membership(d2, it["to"])]}
    r = rels.get(card["relation"], {})
    card["chain_authorizes"] = r.get("chain_authorizes")
    card["relation_labels"] = {k: r.get(k) for k in ("label_es", "label_en", "es", "en", "labels") if k in r}
    cards.append(card)
json.dump(cards, open(HERE / "r5-cards.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
print(len(cards), "cards")
