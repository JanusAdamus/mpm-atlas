"""Juez A r2: etiqueta que pinta el mapa (tiles lang=es) frente a labels.es / labels.en del registro."""
import http.client, json, sqlite3, sys, math, urllib.parse
base = urllib.parse.urlsplit(sys.argv[1])
conn = http.client.HTTPConnection(base.hostname, base.port, timeout=60)
nodes = {}
for z in range(3):
    for x in range(2 ** z):
        for y in range(2 ** z):
            conn.request("GET", f"/api/map/tiles?layer=structure&z={z}&x={x}&y={y}&format=json&lang=es")
            nodes.update({n["id"]: n for n in json.loads(conn.getresponse().read())["nodes"]})
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
ids = sorted(nodes); lab = {}
for i in range(0, len(ids), 900):
    p = ids[i:i + 900]
    for eid, d in c.execute(f"select id, data from entities where id in ({','.join('?' * len(p))})", p):
        lab[eid] = json.loads(d).get("labels", {})
pres = {t: json.loads(p).get("text") for t, p in c.execute("select target_id, payload from curation where undone_at is null and kind='presentation_label'") if json.loads(p).get("lang") == "es"}
shown_es = sum(1 for i in ids if nodes[i]["label"] in {lab[i].get("es"), pres.get(i)} - {None})
shown_en_only = sum(1 for i in ids if "es" not in lab[i] and i not in pres)
n = len(ids); k = shown_es; zz = 1.96; ph = k / n
den = 1 + zz*zz/n; ctr = (ph + zz*zz/(2*n)) / den; hw = zz*math.sqrt(ph*(1-ph)/n + zz*zz/(4*n*n)) / den
print(json.dumps({"n": n, "label_shown_is_es": k, "share": round(ph, 4), "wilson95": [round(ctr-hw, 4), round(ctr+hw, 4)],
                  "no_es_in_registry": shown_en_only}, indent=1))
