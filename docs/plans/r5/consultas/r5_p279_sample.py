"""R5 B5-03: count the current special_case_of claims that come from Wikidata P279 and draw 20 with a fixed seed.

python docs/plans/r5/consultas/r5_p279_sample.py   (read-only; judged in docs/plans/r5/p279-opciones.md)
"""
import json, random, sqlite3, sys
sys.stdout.reconfigure(encoding='utf-8')
c=sqlite3.connect('file:C:/MPM/data/atlas.db?mode=ro',uri=True)
cur="('extracted','model_proposed','formally_linked','editorially_reviewed','disputed')"
ids=sorted(r[0] for r in c.execute(f"select id from claims where relation='special_case_of' and evidence_state in {cur} and json_extract(data,'$.provenance[0].locator') like '%#P279'"))
print(len(ids))
def e(x):
    d=json.loads(c.execute("select data from entities where id=?",(x,)).fetchone()[0])
    g=lambda k,l:(d.get(k) or {}).get(l)
    return f"{x} es={g('labels','es')!r} en={g('labels','en')!r} :: {g('descriptions','en')}"
for i,cid in enumerate(random.Random(20261022).sample(ids,20),1):
    p=dict(c.execute("select role,entity_id from claim_participants where claim_id=?",(cid,)).fetchall())
    print(f"P{i} {cid}\n   S {e(p['subject'])}\n   O {e(p['object'])}")
