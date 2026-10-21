"""R5 integrator, step 5: the general layer (z0-z2) of the PUBLISHED snapshot, read from a running server.

    python -I r5_integrator_measure.py http://127.0.0.1:8765

Same definitions as r5b_names_after.py: a node has a Spanish label if its registry entity has `labels.es`
(derived Mathlib zones excluded: their «(Mathlib, derivado)» label is English words) or an active
`presentation_label` in es.  Derived = id `concept:Mathlib/…`.  Out-of-domain = atlas.ingest.curation.OUT_OF_DOMAIN.
Reads data/atlas.db read-only; GET-only on the server.
"""
import http.client
import json
import sqlite3
import sys
import urllib.parse

sys.path.insert(0, "C:/MPM")
from atlas.ingest.curation import OUT_OF_DOMAIN  # noqa: E402

base = urllib.parse.urlsplit(sys.argv[1] if len(sys.argv) > 1 else "http://127.0.0.1:8765")
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
nodes, versions = {}, set()
for layer in ("structure",):
    for z in range(3):
        for x in range(2 ** z):
            for y in range(2 ** z):
                conn.request("GET", f"/api/map/tiles?layer={layer}&z={z}&x={x}&y={y}&format=json")
                t = json.loads(conn.getresponse().read())
                versions.add(t["layout_version"])
                nodes.update({n["id"]: n for n in t["nodes"]})
ids = sorted(nodes)
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
labels = {}
for i in range(0, len(ids), 900):
    part = ids[i:i + 900]
    for eid, d in c.execute(f"select id, data from entities where id in ({','.join('?' * len(part))})", part):
        labels[eid] = json.loads(d).get("labels", {})
pres_es = {t for t, p in c.execute("select target_id, payload from curation where undone_at is null and kind='presentation_label'")
           if json.loads(p).get("lang") == "es"}
derived = [i for i in ids if i.startswith("concept:Mathlib/")]
es = [i for i in ids if i not in derived and ("es" in labels.get(i, {}) or i in pres_es)]
ood = sorted(set(ids) & {"wd:" + q for q in OUT_OF_DOMAIN})
print(json.dumps({"layout_version": sorted(versions), "nodes_z0_z2": len(ids), "with_es": len(es),
                  "with_es_pct": round(100 * len(es) / max(1, len(ids)), 1), "derived_in_z0_z2": len(derived),
                  "out_of_domain_in_z0_z2": ood, "without_es_sample": [i for i in ids if i not in es][:15]},
                 ensure_ascii=False, indent=1))
