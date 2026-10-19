"""R5 diagnosis: odd-name classes over every unmerged concept/area entity of the registry (read-only)."""
import json, re, sqlite3, unicodedata
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).parent
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
G = {n["id"] for n in json.load(open(HERE / "r5-general.json", encoding="utf-8"))["nodes"]}
NONMATH = re.compile(r"elemento qu[ií]mico|chemical element|unidad de|unit of|unidad b[aá]sica|SI base unit|"
                     r"is[oó]topo|isotope|part[ií]cula subat|subatomic particle|pa[ií]s|country|ciudad|city|"
                     r"ajedrez|chess|planeta|planet|enfermedad|disease|empresa|company|banda|band\b|pel[ií]cula|film", re.I)
cls = defaultdict(list)
n = 0
for eid, typ, data in c.execute("select id, type, data from entities where type in ('concept','area') and merged_into is null"):
    n += 1
    e = json.loads(data)
    lab = e["labels"]; es = lab.get("es"); en = lab.get("en")
    show = es or en or next(iter(lab.values()))
    descr = " ".join((e.get("descriptions") or {}).values())
    row = (eid, show, en, descr[:80], eid in G)
    if re.fullmatch(r"[\d\s.,⁰¹²³⁴⁵⁶⁷⁸⁹×^+\-]+", show) or re.fullmatch(r"\d+", show.split()[0] if show.split() else ""):
        cls["numeric_label"].append(row)
    if NONMATH.search(descr):
        cls["nonmath_descr"].append(row)
    if "›" in show:
        cls["derived_zone_label"].append(row)
    if es is None:
        cls["no_es_label"].append(row)
    if en is None:
        cls["no_en_label"].append(row)
    if "…" in show or show.endswith("..."):
        cls["truncated"].append(row)
print("concept+area unmerged", n)
out = {}
for k, v in sorted(cls.items()):
    ing = sum(1 for r in v if r[4])
    out[k] = {"n": len(v), "in_general": ing, "examples": v[:400]}
    print(k, len(v), "in general", ing)
json.dump(out, open(HERE / "r5-allconcepts.json", "w", encoding="utf-8"), ensure_ascii=False, indent=1)
for q in ("wd:Q556", "wd:Q11570"):
    r = c.execute("select type, data from entities where id=?", (q,)).fetchone()
    m = c.execute("select relation, evidence_state, count(*) from claim_participants cp join claims cl on cl.id=cp.claim_id "
                  "where cp.entity_id=? group by 1,2", (q,)).fetchall()
    print(q, r[0], json.loads(r[1])["labels"], q in G, m)
