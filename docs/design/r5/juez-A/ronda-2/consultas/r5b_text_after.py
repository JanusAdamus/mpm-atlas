"""R5 step 2b: Wikipedia extracts after the sentence-boundary repair, and what the fichas can quote (read-only).

    python -I r5b_text_after.py      # needs r5-general.json (r5_fetch_tiles.py)
"""
import json
import re
import sqlite3
from collections import Counter
from pathlib import Path

HERE = Path(__file__).parent
MAX = 1500
END = re.compile(r"[.!?…][\"»”')\]]*$")
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
n = Counter()
for (d,) in c.execute("select data from entities where type='source_fragment' and data like '%\"Wikipedia (%'"):
    d = json.loads(d)
    for lang, text in (d.get("descriptions") or {}).items():
        n["extracts"] += 1
        n["at_or_over_1500"] += len(text) >= MAX
        n["ends_in_ellipsis"] += text.endswith(" …")
        n["not_ending_a_sentence"] += not END.search(text) and not text.endswith(" …")
        n[f"version:{d['provenance'][0]['extractor_version']}"] += 1
print(dict(n))

# fichas: concepts of the general layer (map:general not excluded) with a complete cited extract
g = json.load(open(HERE / "r5-general.json", encoding="utf-8"))
ids = [x["id"] for x in g["nodes"]]
excl = {t for t, p in c.execute("select target_id, payload from curation where undone_at is null "
                                "and kind='exclude_presentation'") if "map:general" in json.loads(p)["scopes"]}
kept = [i for i in ids if i not in excl]
lead = {}
q = ("select cp.entity_id, e.data from claim_participants cp join claims cl on cl.id=cp.claim_id "
     "join claim_participants co on co.claim_id=cl.id and co.role='object' join entities e on e.id=co.entity_id "
     "where cl.relation='appears_in' and cl.evidence_state in ('extracted','editorially_reviewed') "
     "and cp.role='subject' and e.type='source_fragment' and cp.entity_id in ({})")
for k in range(0, len(kept), 900):
    part = kept[k:k + 900]
    for eid, d in c.execute(q.format(",".join("?" * len(part))), part):
        d = json.loads(d)
        p = d["provenance"][0]
        for lang, text in (d.get("descriptions") or {}).items():
            if p["source_id"].startswith("src:wikipedia") and "oldid=" in p["locator"] and END.search(text):
                lead.setdefault(eid, set()).add(lang)
types = Counter(t for (t,) in [c.execute("select type from entities where id=?", (i,)).fetchone() for i in lead])
print("general-layer nodes kept", len(kept), "| with a complete cited Wikipedia extract", len(lead),
      "| ES", sum("es" in v for v in lead.values()), "| EN only", sum(v == {"en"} for v in lead.values()),
      "| by type", dict(types))
