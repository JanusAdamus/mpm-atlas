"""Relabel route items whose reference path crosses a claim that changed state after the split was frozen.

    python -m bench.relabel --source v3 --target v3.1 [--db data/atlas.db]

The new gold is derived from the registry and the composition table, never from an engine output:
a superseded step is replaced by its live successor (same subject, targets, source_id and locator, or the
item is reported and left alone); a step on a `disputed` claim is excluded by R8 (the default objective does
not opt in), so the reference path concludes nothing and the item becomes an abstention item.  Labels are
then recomputed with `silver.route_labels`, the rule that produced them.  Only silver route items are
candidates (hand-written steps carry no claim ids).  The source version is never edited: the target gets
its own split files, manifest (same membership, new hashes) and a per-item changelog.
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
from dataclasses import replace
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Sequence

from atlas.model import Claim, EvidenceState

from . import compose
from .items import Item, item_hash, read_jsonl, write_jsonl
from .registry import Registry
from .silver import route_labels
from .splits import SPLITS, _set_hash

BENCH = Path(__file__).resolve().parent
ROOT = BENCH.parent
WHY = "ADR-0005 (M2, atlas/ingest/wikidata_m2.py) changed the state of a claim on the reference path"


def _sources(c: Claim) -> set[tuple[str, str]]:
    return {(p.source_id, p.locator) for p in c.provenance}


def _events(con: sqlite3.Connection, cid: str) -> list[dict]:
    rows = con.execute("SELECT seq, actor, kind, payload FROM events WHERE target_id = ? AND kind != 'claim_added' "
                       "ORDER BY seq", (cid,)).fetchall()
    return [{"seq": s, "actor": a, "kind": k, **json.loads(p)} for s, a, k, p in rows]


def _new_step(step: dict, reg: Registry, con: sqlite3.Connection) -> tuple[dict, dict | None, str | None]:
    """(new step, change record or None, problem or None).  Problem = the chain could not be followed."""
    cid = step.get("claim_id")
    c = reg.claim(cid) if cid else None
    if c is None or (c.evidence_state is not EvidenceState.SUPERSEDED and c.evidence_state is not
                     EvidenceState.DISPUTED):
        return step, None, None
    change: dict[str, Any] = {"claim_id": cid, "old_relation": step["relation"], "events": _events(con, cid)}
    if c.evidence_state is EvidenceState.SUPERSEDED:
        succ = reg.claim(reg.live_successor(cid) or "")
        if succ is None:
            return step, None, f"{cid}: superseded without a live successor"
        same = (reg.canonical(succ.subject) == reg.canonical(c.subject)
                and {reg.canonical(t) for t in succ.targets} == {reg.canonical(t) for t in c.targets}
                and _sources(succ) == _sources(c))
        if not same:
            return step, None, f"{cid}: successor {succ.id} differs in endpoints, source_id or locator"
        change.update(successor=succ.id, new_relation=succ.relation.value)
        c = succ
    excluded = c.evidence_state is EvidenceState.DISPUTED  # R8: disputed only if the user opts in
    change.update(state=c.evidence_state.value, excluded_by_r8=excluded)
    hop = compose.Hop(c.relation.value, step.get("direction", "forward"))
    valid = not excluded and compose.step(hop)[0] != "none"
    why = f"claim {c.id} ({c.evidence_state.value}{'; excluded by R8' if excluded else ''})"
    return {**step, "relation": c.relation.value, "claim_id": c.id, "label": "valid" if valid else "invalid",
            "why": why}, change, None


def relabel_item(it: Item, reg: Registry, con: sqlite3.Connection) -> tuple[Item, dict | None, list[str]]:
    """(item, changelog entry or None, problems).  Unaffected items come back unchanged (same object)."""
    p = it.payload
    if it.task != "route" or it.origin != "silver":
        return it, None, []
    got = [_new_step(s, reg, con) for s in p.get("steps") or []]
    problems = [pr for _, _, pr in got if pr]
    changes = [ch for _, ch, _ in got if ch]
    if problems or not changes:
        return it, None, problems
    steps = [s for s, _, _ in got]
    excluded = any(ch["excluded_by_r8"] for ch in changes)
    oracle = "none" if excluded else compose.conclude([compose.Hop(s["relation"], s.get("direction", "forward"))
                                                       for s in steps])
    allowed, forbidden = route_labels(oracle)
    attack = None if allowed else (p.get("attack") or it.strata.get("attack"))
    payload = {**p, "steps": steps, "answerable": bool(allowed), "allowed": allowed, "forbidden": forbidden,
               "attack": attack}
    strata = {**it.strata, "relation": steps[0]["relation"], "adversarial": not allowed}
    new = replace(it, payload=payload, strata=strata,
                  notes=f"{it.notes} | relabeled under ADR-0005 from the registry + composition v"
                        f"{compose.table()['version']}; see changelog")
    entry = {"id": it.id, "why": WHY, "steps_changed": changes,
             "old": {k: p.get(k) for k in ("answerable", "allowed", "forbidden", "attack")},
             "new": {k: payload[k] for k in ("answerable", "allowed", "forbidden", "attack")},
             "old_relations": [s["relation"] for s in p.get("steps") or []],
             "new_relations": [s["relation"] for s in steps],
             "became_abstention": bool(p["answerable"]) and not allowed}
    return new, entry, []


def relabel(source: str, target: str, db: Path) -> dict:
    src_man = json.loads((BENCH / "manifest" / f"{source}.json").read_text(encoding="utf-8"))
    out_dir, man_path = BENCH / "splits" / target, BENCH / "manifest" / f"{target}.json"
    if out_dir.exists() or man_path.exists():
        raise SystemExit(f"{target} already exists: a split version is never rewritten")
    reg = Registry.open(db)
    con = sqlite3.connect(f"file:{Path(db).as_posix()}?mode=ro", uri=True)
    log: list[dict] = []
    problems: list[str] = []
    man = {**src_man, "version": target, "created": datetime.now(timezone.utc).isoformat(),
           "derived_from": {"version": source, "registry": reg.fingerprint(),
                            "composition_version": compose.table()["version"],
                            "changelog": f"bench/manifest/{target}-changelog.jsonl", "rule": __doc__.split("\n\n")[2]},
           "splits": {}}
    try:
        for name in SPLITS:
            items = read_jsonl(BENCH / "splits" / source / f"{name}.jsonl")
            new: list[Item] = []
            for it in items:
                n, entry, probs = relabel_item(it, reg, con)
                new.append(n)
                problems += [f"{name}/{it.id}: {pr}" for pr in probs]
                if entry:
                    log.append({"split": name, **entry})
            write_jsonl(out_dir / f"{name}.jsonl", new)
            hashes = {m.id: item_hash(m) for m in new}
            man["splits"][name] = {**src_man["splits"][name], "file": f"bench/splits/{target}/{name}.jsonl",
                                   "items": hashes, "set_sha256": _set_hash(hashes.values())}
    finally:
        con.close()
        reg.close()
    man["derived_from"]["unresolved_chains"] = problems
    man_path.write_text(json.dumps(man, ensure_ascii=False, indent=1, sort_keys=True), encoding="utf-8")
    with open(BENCH / "manifest" / f"{target}-changelog.jsonl", "w", encoding="utf-8", newline="\n") as fh:
        for e in log:
            fh.write(json.dumps(e, ensure_ascii=False, sort_keys=True) + "\n")
    return {"changed": len(log), "by_split": {s: sum(e["split"] == s for e in log) for s in SPLITS},
            "became_abstention": sum(e["became_abstention"] for e in log), "unresolved_chains": problems}


def main(argv: Sequence[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--source", default="v3")
    ap.add_argument("--target", default="v3.1")
    ap.add_argument("--db", default=str(ROOT / "data" / "atlas.db"))
    a = ap.parse_args(argv)
    print(json.dumps(relabel(a.source, a.target, Path(a.db)), ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
