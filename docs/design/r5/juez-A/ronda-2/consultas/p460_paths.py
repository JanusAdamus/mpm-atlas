"""Juez A r2: ¿alguna ruta servida encadena dos said_equivalent_to?  Pares a~b~c del registro -> GET /api/path a c."""
import http.client, json, sqlite3, sys, urllib.parse
from collections import defaultdict
base = urllib.parse.urlsplit(sys.argv[1])
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
adj = defaultdict(set)
for s, o in c.execute("""select s.entity_id, o.entity_id from claims cl
    join claim_participants s on s.claim_id=cl.id and s.role='subject'
    join claim_participants o on o.claim_id=cl.id and o.role='object'
    where cl.relation='said_equivalent_to' and cl.evidence_state in ('extracted','model_proposed','editorially_reviewed','formally_linked','disputed')"""):
    adj[s].add(o); adj[o].add(s)
pairs = sorted({(a, cc) for b in adj for a in adj[b] for cc in adj[b] if a < cc and cc not in adj[a]})
res = {"pairs_two_hops": len(pairs), "checked": 0, "paths_with_2_said_eq": 0, "status": {}, "examples": []}
for a, cc in pairs:
    for objective in ("shortest", "best_supported"):
        q = urllib.parse.urlencode({"from": a, "to": cc, "objective": objective, "max_hops": 4, "include_disputed": "true", "lang": "es"})
        conn.request("GET", f"/api/path?{q}"); r = conn.getresponse(); body = r.read()
        res["status"][r.status] = res["status"].get(r.status, 0) + 1; res["checked"] += 1
        if r.status != 200: continue
        txt = body.decode("utf-8")
        j = json.loads(txt); res["answered"] = res.get("answered", 0) + bool(j.get("paths")); res["expl_with_2"] = res.get("expl_with_2", 0) + (json.dumps(j.get("exploration")).count("\"said_equivalent_to\"") >= 2)
        for p in j.get("paths", []):
            n = json.dumps(p).count('"said_equivalent_to"')
            if n >= 2:
                res["paths_with_2_said_eq"] += 1; res["examples"].append((a, cc, objective))
print(json.dumps(res, indent=1))
