"""R5 step 2a: the ADR-0005 "Conexiones" checks on the registry after M2 (read-only).

    python -I docs/plans/r5/consultas/r5_m2_check.py [data/atlas.db]
"""
import json, sqlite3, sys
from collections import defaultdict

db = sys.argv[1] if len(sys.argv) > 1 else "data/atlas.db"
c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
CUR = "('extracted','model_proposed','editorially_reviewed','formally_linked','disputed')"


def one(sql: str) -> int:
    return c.execute(sql).fetchone()[0]


def from_pid(rel: str, pid: str) -> int:
    return one(f"""SELECT COUNT(DISTINCT cl.id) FROM claims cl JOIN provenance p ON p.claim_id = cl.id
        WHERE cl.relation = '{rel}' AND cl.evidence_state IN {CUR} AND p.locator LIKE '%#{pid}'""")


# pairs in both directions (same rule as atlas/ingest/wikidata_m2.bidirectional_pairs)
rows = c.execute(f"""SELECT cl.id, cl.relation, cl.evidence_state, s.entity_id, t.entity_id FROM claims cl
    JOIN claim_participants s ON s.claim_id = cl.id AND s.role = 'subject'
    JOIN claim_participants t ON t.claim_id = cl.id AND t.role = 'object'
    WHERE cl.relation IN ('generalizes','special_case_of','prerequisite_for') AND cl.evidence_state IN {CUR}""").fetchall()
key = defaultdict(list)
for cid, rel, st, s, t in rows:
    fam, ab = ("prereq", (s, t)) if rel == "prerequisite_for" else ("general", (s, t) if rel == "generalizes" else (t, s))
    key[(fam, *ab)].append((cid, st))
paired = {x for (f, a, b), v in key.items() if a != b and (f, b, a) in key for x in v}
out = {
    "user_version": one("PRAGMA user_version"),
    "max_event_seq": one("SELECT MAX(seq) FROM events"),
    "equivalent_to_current_from_P460": from_pid("equivalent_to", "P460"),
    "special_case_of_current_from_P31": from_pid("special_case_of", "P31"),
    "part_of_area_formally_linked": one("SELECT COUNT(*) FROM claims WHERE relation='part_of_area' AND evidence_state='formally_linked'"),
    "said_equivalent_to_current": one(f"SELECT COUNT(*) FROM claims WHERE relation='said_equivalent_to' AND evidence_state IN {CUR}"),
    "instance_of_current": one(f"SELECT COUNT(*) FROM claims WHERE relation='instance_of' AND evidence_state IN {CUR}"),
    "special_case_of_current_from_P279": from_pid("special_case_of", "P279"),
    "bidirectional_claims": len(paired),
    "bidirectional_claims_not_disputed": sum(1 for _, st in paired if st != "disputed"),
    "disputed_total": one("SELECT COUNT(*) FROM claims WHERE evidence_state='disputed'"),
    "events_by_m2_actor": dict(c.execute("""SELECT actor || ' ' || kind, COUNT(*) FROM events
        WHERE actor IN ('atlas.ingest.wikidata_m2','curation:bidirectional/1') GROUP BY 1""").fetchall()),
}
print(json.dumps(out, indent=1))
