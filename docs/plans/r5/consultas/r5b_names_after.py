"""R5 step 2b: names on the general layer before/after the curation (read-only).

    python -I r5_fetch_tiles.py      # r5-general.json (the 3,229 nodes of z0-z2)
    python -I r5b_names_after.py     # -> prints the table; reads data/atlas.db read-only

"before" = the source labels (entities.data) of all 3,229 nodes, i.e. diagnóstico §1 [Q2].
"after"  = nodes left once active `exclude_presentation` rows with scope map:general are applied; a node
           counts as having a Spanish label if `labels.es` exists or an active `presentation_label` (lang es)
           exists.  The layout itself still has to honour the flags (map worker): this measures the registry.
"""
import json
import sqlite3
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, "C:/MPM")  # atlas.ingest.curation: the out-of-domain list
HERE = Path(__file__).parent
g = json.load(open(HERE / "r5-general.json", encoding="utf-8"))
ids = [n["id"] for n in g["nodes"]]
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
data = {}
for i in range(0, len(ids), 900):
    part = ids[i:i + 900]
    for eid, d in c.execute(f"select id, data from entities where id in ({','.join('?' * len(part))})", part):
        data[eid] = json.loads(d)
excl, lab_es, by_rule = set(), set(), Counter()
for kind, t, p, rule in c.execute("select kind, target_id, payload, rule from curation where undone_at is null"):
    p = json.loads(p)
    if kind == "exclude_presentation" and "map:general" in p["scopes"]:
        excl.add(t)
    if kind == "presentation_label" and p["lang"] == "es":
        lab_es.add(t)
        by_rule[rule] += t in data
OOD = {"wd:" + q for q in __import__("atlas.ingest.curation", fromlist=["x"]).OUT_OF_DOMAIN}
derived = {i for i in ids if i.startswith("concept:Mathlib/")}
has_es_src = {i for i in ids if "es" in data[i]["labels"]}
after = [i for i in ids if i not in excl]
es_after = [i for i in after if i in has_es_src or i in lab_es]
print("general layer nodes", len(ids))
real_es = has_es_src - derived  # a derived label "x › y (Mathlib, derivado)" is English words with a Spanish tag
print("before (registry labels, no curation): real ES label", len(real_es), f"{len(real_es) / len(ids):.1%}",
      "| no ES label", len(ids) - len(has_es_src), "| derived", len(derived))
print("excluded from map:general", len(excl & set(ids)), "| derived excluded", len(derived & excl),
      "| out-of-domain list in layer", len(OOD & set(ids)), "excluded", len(OOD & set(ids) & excl))
print("after: nodes", len(after), "| with ES (label or presentation)", len(es_after), f"{len(es_after) / len(after):.1%}",
      "| without ES", len(after) - len(es_after), "| derived left", len(derived & set(after)))
print("ES presentation labels on layer nodes by rule", dict(by_rule))
srcfix = Counter(p.get("extractor_version") for i in after for p in data[i].get("provenance", [])
                 if p.get("extractor") == "atlas.ingest.labels_es")
print("ES labels added to layer nodes by labels_es", dict(srcfix))
need = int(0.8 * len(after) + 0.9999) - len(es_after)
print("missing for the 80 % goal", max(0, need))

queue = HERE / "r5b-cola-traduccion-es.tsv"
with open(queue, "w", encoding="utf-8") as fh:
    fh.write("id\ttipo\tetiqueta_en\tdescripcion_en\n")
    for i in sorted(set(after) - set(es_after)):
        d = data[i]
        fh.write(f"{i}\t{d['type']}\t{d['labels'].get('en', '')}\t{d.get('descriptions', {}).get('en', '')}\n")
print("translation queue", queue.name, len(set(after) - set(es_after)))
