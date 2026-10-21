"""Juez B: names in the general layer as the ES map tiles show them (read-only).

    python -I b_names.py  -> b-names.json next to this file

Uses r5-general.json (tiles z0-z2 of the running server) and r5-load.pkl (registry entities).
"""
import json, pickle, sqlite3, sys
from pathlib import Path

HERE = Path(__file__).parent
g = json.load(open(HERE / "r5-general.json", encoding="utf-8"))
ents = pickle.load(open(HERE / "r5-load.pkl", "rb"))["ents"]
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
# the diagnosis list (diagnostico-calidad.md §1, [Q5]), copied by hand
OOD = ("wd:Q1055911 wd:Q1137224 wd:Q137 wd:Q20819848 wd:Q215599 wd:Q2590761 wd:Q842046 wd:Q970141 wd:Q332225 "
       "wd:Q3984344 wd:Q828074 wd:Q810 wd:Q35 wd:Q30 wd:Q145 wd:Q258 wd:Q11379 wd:Q42213 wd:Q589 wd:Q209082 "
       "wd:Q83371 wd:Q9128 wd:Q4173969 wd:Q304479 wd:Q12142910 wd:Q186152 wd:Q387015 wd:Q858684").split()
curated_ood = {r[0] for r in c.execute(
    "select target_id from curation where kind='exclude_presentation' and rule='out-of-domain' and undone_at is null")}
curated_derived = {r[0] for r in c.execute(
    "select target_id from curation where kind='exclude_presentation' and rule='derived-out-of-general' and undone_at is null")}
nodes = g["nodes"]
ids = [n["id"] for n in nodes]
shown = {n["id"]: n["label"] for n in nodes}


def has_es(eid: str) -> bool:
    e = ents[eid][0]
    es = (e.get("labels") or {}).get("es")
    return bool(es) and shown[eid] == es


derived = [i for i in ids if i.startswith("concept:")]
with_es = [i for i in ids if has_es(i)]
derived_label = [i for i in ids if "(Mathlib, derivado)" in shown[i]]
out = {
    "layout_version": g["layout_version"],
    "general_nodes": len(ids),
    "label_shown_equals_registry_es": len(with_es),
    "share_es": round(len(with_es) / len(ids), 4),
    "derived_concept_nodes": len(derived),
    "labels_with_mathlib_derivado": len(derived_label),
    "ood_diagnosis_list_in_layer": [(i, shown[i]) for i in OOD if i in shown],
    "curated_out_of_domain_still_in_layer": len(curated_ood & set(ids)),
    "curated_out_of_domain_total": len(curated_ood),
    "curated_derived_still_in_layer": len(curated_derived & set(ids)),
    "examples_no_es": [(i, shown[i]) for i in ids if not has_es(i)][:40],
}
json.dump(out, open(HERE / "b-names.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
sys.stdout.reconfigure(encoding="utf-8")
print(json.dumps({k: v for k, v in out.items() if k != "examples_no_es"}, ensure_ascii=False, indent=1))
