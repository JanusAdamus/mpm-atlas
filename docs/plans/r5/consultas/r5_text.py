"""R5 diagnosis, stage 4: explanatory text available today for the 3,229 general-layer nodes (read-only)."""
import json, pickle, re, sqlite3, statistics as S
from collections import Counter, defaultdict
from pathlib import Path

HERE = Path(__file__).parent
CURRENT = {"extracted", "model_proposed", "editorially_reviewed", "formally_linked", "disputed"}
d = pickle.load(open(HERE / "r5-load.pkl", "rb"))
ents, rows, parts = d["ents"], d["rows"], d["parts"]
c = sqlite3.connect("file:C:/MPM/data/atlas.db?mode=ro", uri=True)
words = lambda s: len(re.findall(r"\w+", s or ""))

frag_of = defaultdict(list); members = defaultdict(list); formal = defaultdict(list)
for eid, role, cid, rel, st in rows:
    if st not in CURRENT:
        continue
    others = [e for _, r, e in parts[cid] if e != eid]
    if rel == "appears_in" and role == "subject":
        frag_of[eid] += [e for e in others if e.startswith("wp")]
    elif rel == "member_of_concept" and role == "object":
        members[eid] += others
    elif rel == "formalized_as" and role == "subject":
        formal[eid] += [(e, st) for e in others]
need = sorted({f for v in frag_of.values() for f in v} | {m for v in members.values() for m in v} | {f for v in formal.values() for f, _ in v})
desc = {}
for i in range(0, len(need), 900):
    ch = need[i:i + 900]
    for eid, data in c.execute(f"select id, data from entities where id in ({','.join('?' * len(ch))})", ch):
        desc[eid] = json.loads(data).get("descriptions") or {}
print("fragment/member ids", len(need), "loaded", len(desc))

res = {}
for eid, (e, _) in ents.items():
    ds = e.get("descriptions") or {}
    fr = {}
    for f in frag_of.get(eid, []):
        for lang, t in desc.get(f, {}).items():
            fr[lang] = max(fr.get(lang, 0), words(t))
    mem = members.get(eid, [])
    mem_doc = [words(desc.get(m, {}).get("en")) for m in mem if desc.get(m, {}).get("en")]
    fdoc = [words(desc.get(f, {}).get("en")) for f, st in formal.get(eid, []) if desc.get(f, {}).get("en")]
    res[eid] = {"type": e["type"], "derived": eid.startswith("concept:"),
                "wd_es": words(ds.get("es")) if not eid.startswith("concept:") else 0,
                "wd_en": words(ds.get("en")) if not eid.startswith("concept:") else 0,
                "wp_es": fr.get("es", 0), "wp_en": fr.get("en", 0),
                "members": len(mem), "members_with_doc": len(mem_doc), "best_member_doc": max(mem_doc, default=0),
                "formal_links": len(formal.get(eid, [])), "formal_doc": max(fdoc, default=0),
                "formula": bool(e.get("formula"))}
json.dump(res, open(HERE / "r5-text.json", "w", encoding="utf-8"))

def summ(name, xs):
    xs = list(xs); nz = [x for x in xs if x > 0]
    med = S.median(nz) if nz else 0
    q = sorted(nz); p90 = q[int(0.9 * (len(q) - 1))] if q else 0
    print(f"{name:28s} with text {len(nz):5d}/{len(xs)}  median words {med}  p90 {p90}")

for label, sel in (("all G", lambda r: True), ("wd (non-derived)", lambda r: not r["derived"]), ("derived concept:", lambda r: r["derived"])):
    rs = [r for r in res.values() if sel(r)]
    print("==", label, len(rs))
    for k in ("wd_es", "wd_en", "wp_es", "wp_en", "best_member_doc", "formal_doc"):
        summ(k, (r[k] for r in rs))
    print("formula", sum(r["formula"] for r in rs))
    best_cat = [max(r["wd_es"], r["wd_en"]) for r in rs]
    only_short = [r for r in rs if max(r["wd_es"], r["wd_en"]) < 6 and r["wp_es"] == 0 and r["wp_en"] == 0]
    print("catalog description < 6 words (best of es/en):", sum(1 for b in best_cat if b < 6),
          "| ... and no Wikipedia extract at all:", len(only_short),
          "| ... and no Mathlib docstring either:", sum(1 for r in only_short if r["best_member_doc"] == 0 and r["formal_doc"] == 0))
    print("es catalog < 6 words:", sum(1 for r in rs if r["wd_es"] < 6), "| no es Wikipedia extract:", sum(1 for r in rs if r["wp_es"] == 0),
          "| no es text of any kind beyond <6-word catalog:", sum(1 for r in rs if r["wd_es"] < 6 and r["wp_es"] == 0))
    print("has some Wikipedia extract (es or en):", sum(1 for r in rs if r["wp_es"] or r["wp_en"]))
