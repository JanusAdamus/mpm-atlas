"""R5 diagnosis, stage 1: load general-layer entities and their current claims (read-only)."""
import json, pickle, sqlite3, sys
from pathlib import Path

HERE = Path(__file__).parent
DB = "file:C:/MPM/data/atlas.db?mode=ro"
CURRENT = ("extracted", "model_proposed", "editorially_reviewed", "formally_linked", "disputed")

g = json.load(open(HERE / "r5-general.json", encoding="utf-8"))
ids = [n["id"] for n in g["nodes"]]
c = sqlite3.connect(DB, uri=True)
ents = {}
for i in range(0, len(ids), 900):
    chunk = ids[i:i + 900]
    q = f"select id, data, merged_into from entities where id in ({','.join('?' * len(chunk))})"
    for eid, data, merged in c.execute(q, chunk):
        ents[eid] = (json.loads(data), merged)
print("entities loaded", len(ents), "missing", len(set(ids) - set(ents)), file=sys.stderr)

# claims touching G: (claim_id, relation, state, role of G entity, position) + all participants
rows = []
for i in range(0, len(ids), 900):
    chunk = ids[i:i + 900]
    q = (f"select cp.entity_id, cp.role, cl.id, cl.relation, cl.evidence_state from claim_participants cp "
         f"join claims cl on cl.id = cp.claim_id where cp.entity_id in ({','.join('?' * len(chunk))})")
    rows.extend(c.execute(q, chunk).fetchall())
print("claim rows", len(rows), file=sys.stderr)
claim_ids = sorted({r[2] for r in rows})
parts = {}
for i in range(0, len(claim_ids), 900):
    chunk = claim_ids[i:i + 900]
    q = f"select claim_id, role, entity_id, position from claim_participants where claim_id in ({','.join('?' * len(chunk))})"
    for cid, role, eid, pos in c.execute(q, chunk):
        parts.setdefault(cid, []).append((pos, role, eid))
print("participants", len(parts), file=sys.stderr)
pickle.dump({"ents": ents, "rows": rows, "parts": parts}, open(HERE / "r5-load.pkl", "wb"))
