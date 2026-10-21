"""Juez A: nombres y fuera de dominio en z0-z2 de un servidor (lo que ve el lector, lang=es).

    python -I names_check.py http://127.0.0.1:8794 -> imprime JSON
"""
import http.client, json, sqlite3, sys, urllib.parse
sys.path.insert(0, "C:/MPM")
from atlas.ingest.curation import OUT_OF_DOMAIN  # noqa: E402

base = urllib.parse.urlsplit(sys.argv[1])
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
nodes, ver = {}, set()
for z in range(3):
    for x in range(2 ** z):
        for y in range(2 ** z):
            conn.request("GET", f"/api/map/tiles?layer=structure&z={z}&x={x}&y={y}&format=json&lang=es")
            t = json.loads(conn.getresponse().read())
            ver.add(t["layout_version"])
            nodes.update({n["id"]: n for n in t["nodes"]})
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
ids = sorted(nodes)
data = {}
for i in range(0, len(ids), 900):
    part = ids[i:i + 900]
    for eid, d in c.execute(f"select id, data from entities where id in ({','.join('?' * len(part))})", part):
        data[eid] = json.loads(d)
lab_es = {t for k, t, p in c.execute("select kind, target_id, payload from curation where undone_at is null and kind='presentation_label'")
          if json.loads(p).get("lang") == "es"}
derived = [i for i in ids if i.startswith("concept:")]
es = [i for i in ids if i not in derived and ("es" in data[i]["labels"] or i in lab_es)]
ood = sorted(set(ids) & {"wd:" + q for q in OUT_OF_DOMAIN})
notes = [i for i in ids if data[i]["type"] == "note"]
out = {"layout_version": sorted(ver), "nodes_z0_z2": len(ids), "with_es_label": len(es), "share_es": round(len(es) / len(ids), 4),
       "derived_in_z0_z2": len(derived), "out_of_domain_list_size": len(OUT_OF_DOMAIN), "out_of_domain_in_z0_z2": len(ood),
       "ood_examples": [(i, nodes[i]["label"]) for i in ood[:10]], "notes_in_z0_z2": len(notes),
       "label_shown_examples_derived": [nodes[i]["label"] for i in derived[:5]]}
print(json.dumps(out, ensure_ascii=False, indent=1))
