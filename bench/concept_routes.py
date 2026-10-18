"""Concept-level route items (plan 2026-10-17 «capa conceptual»): judge `PathResponse.concepts`.

    python -m bench.concept_routes --http http://127.0.0.1:8768 [--items bench/items/concepts] [--out DIR]

Items are atlasbench/1 `route` items with `payload.level == "concept"`.  The judged answer is
the concept route (`concepts.route`), not the entity-level `paths`.  The oracle is the same
composition table as for entity routes (bench/compose.py) over the concept steps, and every
concept step must be backed by a current witness claim and two current memberships in the
registry.  Reported per class with Wilson intervals; labels are agent-written (provisional).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Mapping

from atlas.model import RelationType

from . import compose
from .backends import BackendError, HttpBackend
from .items import Item, iter_dir, read_jsonl
from .metrics import percentiles, proportion
from .registry import Registry

BENCH = Path(__file__).resolve().parent
DEFAULT_ITEMS = BENCH / "items" / "concepts"
DEFAULT_DB = BENCH.parent / "data" / "atlas.db"
MEMBER = "member_of_concept"


def _membership_ok(reg: Registry, m: Mapping[str, Any]) -> bool:
    c = reg.claim(m.get("claim_id", ""))
    return bool(c and reg.is_current(c.id) and c.relation.value == MEMBER
                and reg.canonical(c.subject) == reg.canonical(m["declaration"])
                and reg.canonical(m["concept"]) in {reg.canonical(t) for t in c.targets})


def _step_problems(i: int, s: Mapping[str, Any], reg: Registry) -> list[str]:
    w = s.get("witness") or {}
    c = reg.claim(w.get("claim_id", ""))
    if c is None:
        return [f"step {i}: witness {w.get('claim_id')} not in registry"]
    probs = []
    if not reg.is_current(c.id):
        probs.append(f"step {i}: witness {c.id} is {c.evidence_state.value}")
    if c.relation.value != s["relation"]:
        probs.append(f"step {i}: relation {s['relation']} but witness says {c.relation.value}")
    subj, tgts = reg.canonical(c.subject), {reg.canonical(t) for t in c.targets}
    a, b = reg.canonical(w["from"]), reg.canonical(w["to"])
    ok = (a == subj and b in tgts) if s.get("direction") == "forward" else (b == subj and a in tgts)
    if not ok:
        probs.append(f"step {i}: witness endpoints/direction do not match {c.id}")
    if not w.get("sources"):
        probs.append(f"step {i}: witness without sources")
    mems = w.get("memberships") or []
    placed = {(reg.canonical(m["declaration"]), reg.canonical(m["concept"])) for m in mems if _membership_ok(reg, m)}
    for decl, concept in ((w["from"], s["from"]), (w["to"], s["to"])):
        if (reg.canonical(decl), reg.canonical(concept)) not in placed:
            probs.append(f"step {i}: no current membership {decl} -> {concept}")
    return probs


def structural_problems(level: Mapping[str, Any], reg: Registry) -> list[str]:
    """Registry-checkable defects of one concept route (empty = structurally valid)."""
    steps = (level.get("route") or {}).get("steps") or []
    if not steps:
        return ["empty route"]
    probs = []
    if steps[0]["from"] != level["from"]["concept"] or steps[-1]["to"] != level["to"]["concept"]:
        probs.append("route does not join the endpoints' concepts")
    for end in (level["from"], level["to"]):
        if end["via"] == "primary_concept" and not any(
                _membership_ok(reg, {"claim_id": c.id, "declaration": end["requested"], "concept": end["concept"]})
                for c in reg.store.claims_for(end["requested"], relations=[RelationType.MEMBER_OF_CONCEPT])):
            probs.append(f"{end['requested']}: no current membership in {end['concept']}")
    for i, s in enumerate(steps):
        probs += _step_problems(i, s, reg)
        if i and steps[i - 1]["to"] != s["from"]:
            probs.append(f"step {i}: does not start where step {i - 1} ended")
    return probs


def judge(item: Item, resp: Mapping[str, Any], reg: Registry) -> dict:
    p = item.payload
    level = resp.get("concepts")
    base = {"answerable": p["answerable"], "attack": p.get("attack")}
    if not level:
        return {**base, "error": "response has no concept layer"}
    if level["abstained"]:
        return {**base, "abstained": True, "abstention": level.get("abstention"), "reason": level.get("reason")}
    concl = level["route"]["conclusion"]
    key = "hypothesis" if concl.get("is_hypothesis") else concl["kind"]
    steps = level["route"]["steps"]
    oracle = compose.conclude([compose.Hop(s["relation"], s["direction"]) for s in steps])
    overclaim = compose.strength(key) > compose.strength(oracle)
    return {**base, "abstained": False, "conclusion": key, "oracle": oracle, "overclaim": overclaim,
            "forbidden_violation": key in p["forbidden"], "allowed_hit": key in p["allowed"],
            "structural_problems": structural_problems(level, reg), "hops": len(steps),
            "concepts": [steps[0]["from"]] + [s["to"] for s in steps],
            "witnesses": [s["witness"]["claim_id"] for s in steps],
            "weakest_evidence": concl.get("weakest_evidence")}


def summarize(records: list[dict]) -> dict:
    ok = [r for r in records if "error" not in r]
    ans = [r for r in ok if r["answerable"]]
    adv = [r for r in ok if not r["answerable"] or r["attack"]]
    answered = [r for r in ok if not r.get("abstained")]
    return {
        "items": len(records), "errors": len(records) - len(ok),
        "answerable_allowed_hit": proportion(sum(bool(r.get("allowed_hit")) for r in ans), len(ans)),
        "abstain_on_answerable": proportion(sum(bool(r.get("abstained")) for r in ans), len(ans)),
        "adversarial_no_forbidden": proportion(sum(not r.get("forbidden_violation") for r in adv), len(adv)),
        "abstain_on_unanswerable": proportion(sum(bool(r.get("abstained")) for r in ok if not r["answerable"]),
                                              sum(not r["answerable"] for r in ok)),
        "structurally_valid": proportion(sum(not r["structural_problems"] for r in answered), len(answered)),
        "overclaim_vs_oracle": proportion(sum(r["overclaim"] for r in answered), len(answered)),
    }


def run(items: list[Item], http: HttpBackend, reg: Registry, max_hops: int = 6) -> dict:
    records, ms = [], []
    for it in items:
        p = it.payload
        src, dst = reg.ids(p["from"]), reg.ids(p["to"])
        if not src or not dst:
            records.append({"id": it.id, "answerable": p["answerable"], "attack": p.get("attack"), "error": "unresolved"})
            continue
        try:
            t = http.get("/api/path", {"from": src[0], "to": dst[0], "max_hops": max_hops, "lang": "es",
                                       "objective": p.get("objective", "best_supported")})
        except BackendError as exc:
            records.append({"id": it.id, "answerable": p["answerable"], "attack": p.get("attack"), "error": str(exc)})
            continue
        ms.append(t.ms)
        records.append({"id": it.id, "ms": t.ms, **judge(it, t.value, reg)})
    return {"summary": summarize(records), "latency_ms": percentiles(ms) if ms else None, "records": records}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="python -m bench.concept_routes")
    ap.add_argument("--http", required=True, help="base URL of `atlas serve`")
    ap.add_argument("--items", default=str(DEFAULT_ITEMS))
    ap.add_argument("--db", default=str(DEFAULT_DB))
    ap.add_argument("--out", default=None)
    a = ap.parse_args(argv)
    src = Path(a.items)
    items = [i for i in (list(iter_dir(src)) if src.is_dir() else read_jsonl(src)) if i.payload.get("level") == "concept"]
    rep = run(items, HttpBackend(a.http), Registry.open(a.db))
    rep["health"] = HttpBackend(a.http).get("/api/health", {}).value.get("snapshot")
    text = json.dumps(rep, ensure_ascii=False, indent=1, default=str)
    if a.out:
        Path(a.out).mkdir(parents=True, exist_ok=True)
        (Path(a.out) / "report.json").write_text(text, encoding="utf-8")
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps({"summary": rep["summary"], "latency_ms": rep["latency_ms"]}, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
