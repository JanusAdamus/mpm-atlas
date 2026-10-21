"""Juez B ronda 2: share of ES-shown labels in the general layer, counting curated presentation labels.

    python -I b_names2.py  -> b-names2.json next to this file
Shown label (tiles, ES) vs registry labels: 'es_registry' if equal to labels.es; 'es_curated' if it differs from
labels.en and a presentation_label curation exists for the node; 'en' if it equals labels.en and labels.es is
missing or different; else 'other' (listed for hand reading).
"""
import json, pickle, sqlite3, sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).parent
g = json.load(open(HERE / "r5-general.json", encoding="utf-8"))
E = pickle.load(open(HERE / "r5-load.pkl", "rb"))["ents"]
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
pres = {r[0] for r in c.execute("select target_id from curation where kind='presentation_label' and undone_at is null")}
cls, other = Counter(), []
for n in g["nodes"]:
    lab = E[n["id"]][0].get("labels") or {}
    es, en, shown = lab.get("es"), lab.get("en"), n["label"]
    k = ("es_registry" if es and shown == es else "es_curated" if n["id"] in pres and shown != en
         else "en" if shown == en else "other")
    cls[k] += 1
    if k == "other":
        other.append((n["id"], shown, es, en))
total = len(g["nodes"])
out = {"layout_version": g["layout_version"], "total": total, "classes": dict(cls),
       "share_es_strict": round(cls["es_registry"] / total, 4),
       "share_es_with_curated": round((cls["es_registry"] + cls["es_curated"]) / total, 4),
       "share_es_upper_bound_incl_other": round((total - cls["en"]) / total, 4), "other": other}
json.dump(out, open(HERE / "b-names2.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
sys.stdout.reconfigure(encoding="utf-8")
print({k: v for k, v in out.items() if k != "other"}); print(other[:30])
