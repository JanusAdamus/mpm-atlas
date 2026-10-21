"""Round 5 M2 data transformations of the connections (ADR-0005 §7b-§7e), through the store API.

Each operation is named, re-runnable (a second run finds nothing to do and logs no event) and leaves
the history in events (supersede / set_evidence_state); nothing is deleted.

    python -m atlas.ingest.wikidata_m2 [--db data/atlas.db] [--only p460,p31,bidirectional,part_of_area] [--dry-run]

Order matters: P31 (§7c) runs before the bidirectional rule (§7d), so a P31 instance is never
disputed as if it were a subclass.  Writes go in short transactions (CHUNK operations) and retry
when another writer holds the database.
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
import time
from dataclasses import replace
from typing import Callable, Iterable, Iterator, Sequence

from ..model import Claim, EvidenceState, RelationType
from ..store import Store
from .wikidata import MAPPING_VERSION, claim_id

R = RelationType
CHUNK = 500  # operations per transaction: short, so the parallel writers are not starved
RETRIES = 60
BIDIRECTIONAL_ACTOR = "curation:bidirectional/1"
M2_ACTOR = "atlas.ingest.wikidata_m2"
_CURRENT = "('extracted','model_proposed','editorially_reviewed','formally_linked','disputed')"


def _retrying(store: Store, ops: Sequence[Callable[[], None]]) -> None:
    """Run `ops` in one transaction; on 'database is locked' wait and retry the whole chunk."""
    for attempt in range(RETRIES):
        try:
            with store.transaction():
                for op in ops:
                    op()
            return
        except sqlite3.OperationalError as exc:
            if "locked" not in str(exc) and "busy" not in str(exc):
                raise
            time.sleep(min(5.0, 0.5 * (attempt + 1)))
    raise RuntimeError("database stayed busy; nothing of this chunk was written")


def _chunks(items: Sequence, n: int = CHUNK) -> Iterator[Sequence]:
    for i in range(0, len(items), n):
        yield items[i:i + n]


def _ids(store: Store, sql: str, args: Iterable = ()) -> list[str]:
    return [r[0] for r in store._conn.execute(sql, tuple(args))]  # read-only query, no write


def _wikidata_by_pid(store: Store, relation: RelationType, pid: str) -> list[str]:
    """Current claims of `relation` whose Wikidata provenance is property `pid`."""
    return _ids(store, f"""SELECT DISTINCT c.id FROM claims c JOIN provenance p ON p.claim_id = c.id
        WHERE c.relation = ? AND c.evidence_state IN {_CURRENT} AND p.source_id = 'src:wikidata'
        AND p.locator LIKE ? ORDER BY c.rowid""", (relation.value, f"%#{pid}"))


def _replacement(old: Claim, pid: str, relation: RelationType, *, keep_assumptions: bool) -> Claim:
    qid = old.provenance[0].locator.rsplit("/", 1)[-1].split("#", 1)[0]
    obj = old.targets[0].removeprefix("wd:")
    prov = tuple(replace(p, extractor_version=MAPPING_VERSION) for p in old.provenance)
    return replace(old, id=claim_id(qid, pid, obj, relation), relation=relation, provenance=prov,
                   assumptions=old.assumptions if keep_assumptions else (), supersedes=None, superseded_by=None)


def _supersede_pid(store: Store, old_rel: RelationType, pid: str, new_rel: RelationType, reason: str,
                   *, keep_assumptions: bool, dry_run: bool) -> int:
    todo = _wikidata_by_pid(store, old_rel, pid)
    if dry_run:
        return len(todo)
    for chunk in _chunks(todo):
        olds = [store.get_claim(cid) for cid in chunk]
        _retrying(store, [
            (lambda o=o: store.supersede(o.id, _replacement(o, pid, new_rel, keep_assumptions=keep_assumptions),
                                         actor=M2_ACTOR, reason=reason))
            for o in olds])
    return len(todo)


def p460(store: Store, *, dry_run: bool = False) -> int:
    """§7b: equivalent_to from P460 -> said_equivalent_to (does not compose)."""
    return _supersede_pid(store, R.EQUIVALENT_TO, "P460", R.SAID_EQUIVALENT_TO,
                          "ADR-0005 §7b: Wikidata P460 'said to be the same as' is not a transitive equivalence",
                          keep_assumptions=True, dry_run=dry_run)


def p31(store: Store, *, dry_run: bool = False) -> int:
    """§7c: special_case_of from P31 -> instance_of (P279 stays special_case_of)."""
    return _supersede_pid(store, R.SPECIAL_CASE_OF, "P31", R.INSTANCE_OF,
                          "ADR-0005 §7c: Wikidata P31 'instance of' is an example, not a special case",
                          keep_assumptions=False, dry_run=dry_run)


def bidirectional_pairs(store: Store) -> dict[str, set[str]]:
    """Current claims that contradict another one: A more general than B and B more general than A
    (generalizes, or special_case_of read through its inverse), or prerequisite_for both ways.
    Returns claim id -> ids of its contradicting partners.  depends_on_logically is not included."""
    rows = store._conn.execute(f"""SELECT c.id, c.relation, s.entity_id, t.entity_id FROM claims c
        JOIN claim_participants s ON s.claim_id = c.id AND s.role = 'subject'
        JOIN claim_participants t ON t.claim_id = c.id AND t.role = 'object'
        WHERE c.relation IN ('generalizes','special_case_of','prerequisite_for')
        AND c.evidence_state IN {_CURRENT}""").fetchall()
    by_key: dict[tuple[str, str, str], set[str]] = {}
    for cid, rel, s, t in rows:
        family, (a, b) = ("prereq", (s, t)) if rel == "prerequisite_for" else \
            ("general", (s, t) if rel == "generalizes" else (t, s))
        by_key.setdefault((family, a, b), set()).add(cid)
    out: dict[str, set[str]] = {}
    for (family, a, b), ids in by_key.items():
        partners = by_key.get((family, b, a))
        if a != b and partners:
            for cid in ids:
                out.setdefault(cid, set()).update(partners)
    return out


def bidirectional(store: Store, *, dry_run: bool = False) -> int:
    """§7d: both claims of a contradicting pair -> disputed, citing the partner."""
    pairs = bidirectional_pairs(store)
    states = dict(store._conn.execute(
        f"SELECT id, evidence_state FROM claims WHERE id IN ({','.join('?' * len(pairs))})", tuple(pairs))) \
        if pairs else {}
    todo = sorted(cid for cid in pairs if states[cid] != EvidenceState.DISPUTED.value)
    if dry_run:
        return len(todo)
    for chunk in _chunks(todo):
        _retrying(store, [
            (lambda cid=cid: store.set_evidence_state(
                cid, EvidenceState.DISPUTED, actor=BIDIRECTIONAL_ACTOR,
                reason="ADR-0005 §7d: contradicted by the opposite claim " + ", ".join(sorted(pairs[cid]))))
            for cid in chunk])
    return len(todo)


def part_of_area(store: Store, *, dry_run: bool = False) -> int:
    """§7e: part_of_area formally_linked -> extracted (a file location is not formal verification)."""
    todo = _ids(store, "SELECT id FROM claims WHERE relation = 'part_of_area' AND evidence_state = 'formally_linked' "
                       "ORDER BY rowid")
    if dry_run:
        return len(todo)
    for chunk in _chunks(todo, 2000):
        _retrying(store, [
            (lambda cid=cid: store.set_evidence_state(
                cid, EvidenceState.EXTRACTED, actor=M2_ACTOR,
                reason="ADR-0005 §7e: la ubicación en un archivo no es verificación formal"))
            for cid in chunk])
    return len(todo)


OPERATIONS: dict[str, Callable[..., int]] = {
    "p460": p460, "p31": p31, "bidirectional": bidirectional, "part_of_area": part_of_area}


def run(store: Store, only: Iterable[str] = OPERATIONS, *, dry_run: bool = False) -> dict[str, int]:
    return {name: OPERATIONS[name](store, dry_run=dry_run) for name in OPERATIONS if name in set(only)}


def main(argv: Sequence[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--db", default="data/atlas.db")
    ap.add_argument("--only", default=",".join(OPERATIONS))
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args(argv)
    unknown = set(a.only.split(",")) - set(OPERATIONS)
    if unknown:
        ap.error(f"unknown operations: {sorted(unknown)}")
    with Store(a.db) as store:
        t0 = time.perf_counter()
        out = run(store, a.only.split(","), dry_run=a.dry_run)
    print(json.dumps({"dry_run": a.dry_run, "changed": out, "seconds": round(time.perf_counter() - t0, 1)}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
