"""Juez B: concepts of the published general layer whose card can show a complete cited Wikipedia extract.

    python -I b_extracts.py  -> b-extracts.json next to this file

Complete = text ends a sentence; cited = provenance from src:wikipedia-* with a revision (oldid) locator.
Counted over the layer as the published tiles show it (r5-general.json), no curation filter applied,
because the published snapshot does not apply it.
"""
import json, re, sqlite3, sys
from pathlib import Path

HERE = Path(__file__).parent
END = re.compile(r"[.!?…][\"»”')\]]*$")
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
g = json.load(open(HERE / "r5-general.json", encoding="utf-8"))
ids = [n["id"] for n in g["nodes"]]
typ = {i: c.execute("select type from entities where id=?", (i,)).fetchone()[0] for i in ids}
q = ("select cp.entity_id, e.data from claim_participants cp join claims cl on cl.id=cp.claim_id "
     "join claim_participants co on co.claim_id=cl.id and co.role='object' join entities e on e.id=co.entity_id "
     "where cl.relation='appears_in' and cl.evidence_state in ('extracted','editorially_reviewed') "
     "and cp.role='subject' and e.type='source_fragment' and cp.entity_id in ({})")
lead: dict[str, set] = {}
for k in range(0, len(ids), 900):
    part = ids[k:k + 900]
    for eid, d in c.execute(q.format(",".join("?" * len(part))), part):
        d = json.loads(d)
        p = d["provenance"][0]
        for lang, text in (d.get("descriptions") or {}).items():
            if p["source_id"].startswith("src:wikipedia") and "oldid=" in p["locator"] and END.search(text):
                lead.setdefault(eid, set()).add(lang)
concepts = [i for i in ids if typ[i] == "concept"]
out = {"layout_version": g["layout_version"], "layer_nodes": len(ids), "concept_nodes": len(concepts),
       "concept_nodes_non_derived": sum(1 for i in concepts if not i.startswith("concept:")),
       "nodes_with_complete_cited_extract": len(lead),
       "concepts_with_complete_cited_extract": sum(1 for i in concepts if i in lead),
       "concepts_with_ES_extract": sum(1 for i in concepts if "es" in lead.get(i, ())),
       "concepts_EN_only": sum(1 for i in concepts if lead.get(i) == {"en"})}
json.dump(out, open(HERE / "b-extracts.json", "w", encoding="utf-8"), indent=1)
sys.stdout.reconfigure(encoding="utf-8")
print(out)
