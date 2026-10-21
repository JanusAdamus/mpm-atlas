"""R5 diagnosis, stage 3: what connections the general layer carries (registry claims + concept aggregates)."""
import http.client, json, pickle, sys
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).parent
CURRENT = {"extracted", "model_proposed", "editorially_reviewed", "formally_linked", "disputed"}
MATH = {"depends_on_logically", "proves", "prerequisite_for", "generalizes", "special_case_of", "equivalent_to",
        "analogous_to", "applied_in", "models", "counterexample_to"}
DOC = {"cites", "appears_in"}
GROUP = {"part_of_area", "member_of_concept"}
HIST = {"named_after", "influenced", "authored_by"}
FORMAL = {"formalized_as"}

def cat(r):
    return ("math" if r in MATH else "doc" if r in DOC else "area/grouping" if r in GROUP
            else "history" if r in HIST else "formal" if r in FORMAL else "other")

d = pickle.load(open(HERE / "r5-load.pkl", "rb"))
ents, rows, parts = d["ents"], d["rows"], d["parts"]
G = set(ents)
cur = [r for r in rows if r[4] in CURRENT]
# 1. relation x state, claims touching G (distinct claims)
seen = {}
for eid, role, cid, rel, st in cur:
    seen[cid] = (rel, st)
rs = Counter(seen.values())
print("distinct current claims touching G", len(seen))
for (rel, st), n in sorted(rs.items(), key=lambda kv: -kv[1]):
    print(f"  {rel:22s} {st:18s} {n}")
# 2. ficha: first 200 current claims by id per entity (entity.rs: ORDER BY id LIMIT 200)
per = defaultdict(set)
for eid, role, cid, rel, st in cur:
    per[eid].add(cid)
ficha = Counter(); ficha_ent = Counter()
for eid, cids in per.items():
    shown = sorted(cids)[:200]
    for cid in shown:
        ficha[cat(seen[cid][0])] += 1
        ficha_ent[(seen[cid][0])] += 1
print("ficha view (first 200 by id), by category", dict(ficha), "total", sum(ficha.values()))
print("ficha view by relation", ficha_ent.most_common())
# 3. G entities without any current direct math claim; and with both ends in G
math_ents = Counter(); math_inG = Counter(); any_ent = Counter()
pair_claims = []
for cid, (rel, st) in seen.items():
    ends = [e for _, role, e in parts[cid] if role in ("subject", "object", "premise")]
    for e in ends:
        if e in G:
            any_ent[e] += 1
            if rel in MATH:
                math_ents[e] += 1
    if len(ends) >= 2 and all(e in G for e in ends) and len(set(ends)) >= 2:
        pair_claims.append((cid, rel, st, tuple(ends)))
concepts = [e for e in G if ents[e][0]["type"] == "concept"]
derived = [e for e in concepts if e.startswith("concept:")]
print("G", len(G), "concept-type", len(concepts), "derived concept:", len(derived))
print("G entities with zero direct math claims:", sum(1 for e in G if math_ents[e] == 0),
      "| concept-type:", sum(1 for e in concepts if math_ents[e] == 0),
      "| wd concept-type:", sum(1 for e in concepts if math_ents[e] == 0 and e.startswith("wd:")))
print("claims with >=2 distinct G endpoints:", len(pair_claims), Counter(p[1] for p in pair_claims).most_common())
# 4. concept aggregates from the running server (port from argv)
conn = http.client.HTTPConnection("127.0.0.1", int(sys.argv[1]) if len(sys.argv) > 1 else 8765, timeout=60)
agg = {}
for i, e in enumerate(sorted(concepts)):
    conn.request("GET", f"/api/concept?id={e}&limit=1&edge_limit=200&lang=es")
    resp = conn.getresponse(); body = resp.read()
    if resp.status != 200:
        agg[e] = None; continue
    j = json.loads(body)
    stats = Counter()
    for side in ("edges_out", "edges_in"):
        for ed in j[side]:
            for s in ed["stats"]:
                stats[(side, s["relation"], s["evidence_state"])] += s["count"]
    agg[e] = {"out_total": j["edges_out_total"], "in_total": j["edges_in_total"], "members_total": j["members_total"],
              "stats": [[*k, v] for k, v in stats.items()], "description": j.get("description"),
              "edges_out": [(ed["concept"]["id"], ed["concept"]["label"], ed["total"], ed["stats"][0]["relation"]) for ed in j["edges_out"][:5]]}
print("concept API not 200:", sum(1 for v in agg.values() if v is None), file=sys.stderr)
no_agg = [e for e, v in agg.items() if v and v["out_total"] + v["in_total"] == 0]
no_math_at_all = [e for e in concepts if math_ents[e] == 0 and (agg.get(e) is None or agg[e]["out_total"] + agg[e]["in_total"] == 0)]
print("concept-type with no aggregate concept edge:", len(no_agg), "| no direct math claim AND no aggregate edge:", len(no_math_at_all),
      "| of which wd:", sum(1 for e in no_math_at_all if e.startswith("wd:")))
pickle.dump({"seen": seen, "pair_claims": pair_claims, "agg": agg, "math_ents": math_ents, "any_ent": any_ent,
             "no_math_at_all": no_math_at_all, "ficha": ficha, "ficha_ent": ficha_ent}, open(HERE / "r5-edges.pkl", "wb"))
