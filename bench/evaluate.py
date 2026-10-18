"""Per-component evaluation: run items against backends, return per-item records.

Records are plain dicts so report.py can aggregate them by any stratum.  A backend
that raises ComponentMissing marks the whole component missing for that backend;
nothing here crashes on a missing component.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any, Iterable, Sequence

from atlas.model import RELATIONS, RelationType

from . import compose
from .backends import BackendError, ComponentMissing, PathAnswer
from .items import Item
from .metrics import mrr_at_k, ndcg_at_k, recall_at_k
from .registry import Registry

SEARCH_LIMIT = 50
ENTITY_MAX_CLAIMS = 1000  # the API's maximum `limit` for /api/entity (SPEC §15); there is no offset
PROV_FIELDS = ("source_id", "locator", "extractor", "extractor_version", "license", "retrieved_at")


def _missing(backend: Any, exc: Exception) -> dict:
    return {"backend": backend.name, "missing": str(exc)}


def _grades(item: Item, reg: Registry) -> dict[str, int]:
    grades: dict[str, int] = {}
    for q in item.payload["qrels"]:
        for eid in reg.ids(q["ref"]):
            grades[eid] = max(grades.get(eid, 0), q["grade"])
    return grades


def retrieval(items: Sequence[Item], reg: Registry, backends: Sequence[Any], trivial: set[str]) -> dict:
    out: dict[str, Any] = {"records": [], "unresolved": [], "missing": {}}
    live = list(backends)
    for it in items:
        grades = _grades(it, reg)
        if not any(g >= 2 for g in grades.values()):
            out["unresolved"].append(it.id)
            continue
        p = it.payload
        rec = {"id": it.id, "origin": it.origin, "label_status": it.label_status, "lang": p["lang"], "query": p["query"],
               "qrels_total": len(p["qrels"]),
               "qrels_resolved": sum(bool(reg.ids(q["ref"])) for q in p["qrels"]),
               "modality": p["modality"], "area": it.strata.get("area", "unknown"), "family": it.split_keys["family"],
               "generator": it.strata.get("generator", it.origin), "trivial": it.id in trivial, "by_backend": {}}
        for b in list(live):
            try:
                t = b.search(p["query"], lang=p["lang"], limit=SEARCH_LIMIT, types=p.get("types") or ())
            except ComponentMissing as exc:
                out["missing"][b.name] = str(exc)
                live.remove(b)
                continue
            except BackendError as exc:  # PROTOCOL §6.2 (l): a rejected query is an empty ranking, not a skipped one
                rec["by_backend"][b.name] = {**_rank_metrics([], grades), "failed": str(exc), "top": [], "lanes": [],
                                             "by_lane": {}}
                continue
            ranked = [reg.canonical(x) for x in t.value.ids]
            rec["by_backend"][b.name] = {**_rank_metrics(ranked, grades), "ms": t.ms, "top": ranked[:10],
                                         "lanes": sorted(t.value.lanes_used()),
                                         "by_lane": {lane: _rank_metrics([reg.canonical(x) for x in ids], grades)
                                                     for lane, ids in t.value.by_lane().items()}}
        out["records"].append(rec)
    return out


def _rank_metrics(ranked: Sequence[str], grades: dict[str, int]) -> dict[str, float]:
    return {"ndcg@10": ndcg_at_k(ranked, grades, 10), "recall@10": recall_at_k(ranked, grades, 10),
            "recall@50": recall_at_k(ranked, grades, 50), "mrr@10": mrr_at_k(ranked, grades, 10)}


# ------------------------------------------------------------------ routes

def structural_problems(ans_path: dict, src: str, dst: str, reg: Registry) -> list[str]:
    """Registry-checkable defects of one returned path (empty = structurally valid)."""
    probs: list[str] = []
    steps = ans_path.get("steps") or []
    if not steps:
        return ["empty path"]
    if reg.canonical(steps[0]["from"]) != src or reg.canonical(steps[-1]["to"]) != dst:
        probs.append("path does not join the requested endpoints")
    for i, s in enumerate(steps):
        c = reg.claim(s["claim_id"])
        if c is None:
            probs.append(f"step {i}: claim {s['claim_id']} not in registry")
            continue
        if not reg.is_current(c.id):
            probs.append(f"step {i}: claim {c.id} is {c.evidence_state.value}")
        if c.relation.value != s["relation"]:
            probs.append(f"step {i}: relation {s['relation']} but claim says {c.relation.value}")
        subj, tgts = reg.canonical(c.subject), {reg.canonical(t) for t in c.targets}
        a, b = reg.canonical(s["from"]), reg.canonical(s["to"])
        fwd = a == subj and b in tgts
        inv = b == subj and a in tgts
        if not ((s.get("direction") == "inverse" and inv) or (s.get("direction", "forward") == "forward" and fwd)):
            probs.append(f"step {i}: endpoints/direction do not match claim {c.id}")
        shown = set(s.get("premises") or []) | set(s.get("premises_missing") or [])
        if c.is_conjunction and len(c.targets) > 1 and len(shown) < len(c.targets):
            probs.append(f"step {i}: conjunctive claim {c.id} shown without all its premises (R7)")
        if i and reg.canonical(steps[i - 1]["to"]) != a:
            probs.append(f"step {i}: does not start where step {i - 1} ended")
    return probs


def judge_path(item: Item, ans: PathAnswer, src: str, dst: str, reg: Registry) -> dict:
    p = item.payload
    if ans.abstained or not ans.paths:
        return {"abstained": True, "reason": ans.reason, "answerable": p["answerable"]}
    top = ans.paths[0]
    concl = top.get("conclusion") or {}
    kind = concl.get("kind")  # the engine's composition key, when it reports one; else parse the text
    key = ("hypothesis" if concl.get("is_hypothesis") else kind) if kind in compose.table()["conclusions"] else         compose.normalize_conclusion(concl.get("authorizes"), bool(concl.get("is_hypothesis")))
    oracle = compose.conclude([compose.Hop(s["relation"], s.get("direction", "forward"))
                               for s in top.get("steps") or []
                               if s.get("relation") in {r.value for r in RelationType}])
    probs = structural_problems(top, src, dst, reg)
    overclaim = compose.strength(key) > compose.strength(oracle)
    claims = [s.get("claim_id") for s in top.get("steps") or []]
    ref = [s.get("claim_id") for s in p.get("steps") or []]
    # a silver label only speaks for its own reference path: another path may legitimately conclude more
    other_path = p.get("forbidden_scope") == "reference_path" and claims != ref
    violation = overclaim if other_path else key in p["forbidden"]
    return {"abstained": False, "answerable": p["answerable"], "conclusion": key, "oracle": oracle,
            "complete": concl.get("complete"), "other_path": other_path,
            "structural_problems": probs, "forbidden_violation": violation,
            "allowed_hit": key in p["allowed"], "overclaim": overclaim,
            "hops": len(top.get("steps") or []), "claims": [s.get("claim_id") for s in top.get("steps") or []]}


def routes(items: Sequence[Item], reg: Registry, backends: Sequence[Any], *, max_hops: int = 6) -> dict:
    out: dict[str, Any] = {"records": [], "unresolved": [], "missing": {}}
    live = list(backends)
    for it in items:
        p = it.payload
        src, dst = reg.ids(p["from"]), reg.ids(p["to"])
        if not src or not dst:
            out["unresolved"].append(it.id)
            continue
        rec = {"id": it.id, "origin": it.origin, "label_status": it.label_status,
               "adversarial": not p["answerable"] or bool(p.get("attack")), "attack": p.get("attack"),
               "answerable": p["answerable"], "area": it.strata.get("area", "unknown"),
               "family": it.split_keys["family"], "by_backend": {}}
        for b in list(live):
            try:
                t = b.path(src[0], dst[0], objective=p.get("objective", "best_supported"), max_hops=max_hops)
            except ComponentMissing as exc:
                out["missing"][b.name] = str(exc)
                live.remove(b)
                continue
            except BackendError as exc:
                rec["by_backend"][b.name] = {"error": str(exc)}
                continue
            rec["by_backend"][b.name] = {**judge_path(it, t.value, src[0], dst[0], reg), "ms": t.ms}
        out["records"].append(rec)
    return out


# ------------------------------------------------------------------ entity resolution & relations

def load_queue(path: Path | None) -> set[frozenset[str]]:
    if path is None or not path.exists():
        return set()
    pairs = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            d = json.loads(line)
            if d.get("kind") == "identity":
                pairs.add(frozenset((d["left"], d["right"])))
    return pairs


def er_pairs(items: Sequence[Item], reg: Registry, queue: set[frozenset[str]]) -> dict:
    out: dict[str, Any] = {"records": [], "unresolved": []}
    for it in items:
        p = it.payload
        a, b = reg.ids(p["a"]), reg.ids(p["b"])
        if not a or not b:
            out["unresolved"].append(it.id)
            continue
        raw = frozenset(r.get("id", "") for r in (p["a"], p["b"]))
        pred = "abstain" if raw in queue else ("same" if set(a) & set(b) else "different")
        out["records"].append({"id": it.id, "label": p["label"], "pred": pred, "hardness": p["hardness"],
                               "circular": bool(p.get("circular_with")), "type": it.strata.get("type"),
                               "area": it.strata.get("area", "unknown"), "label_status": it.label_status,
                               "family": it.split_keys["family"]})
    return out


def relations(items: Sequence[Item], reg: Registry) -> dict:
    out: dict[str, Any] = {"records": [], "unresolved": []}
    for it in items:
        p = it.payload
        c = reg.claim(p["claim_id"])
        if c is None:
            out["unresolved"].append(it.id)
            continue
        out["records"].append({"id": it.id, "family": p["family"], "relation": p["relation"], "label": p["label"],
                               "label_status": it.label_status, "current": reg.is_current(c.id),
                               "corroborated": bool(p.get("corroborated_by")), "source": p["source_id"],
                               "priority": RELATIONS[RelationType(p["relation"])].projection.value})
    return out


def sourced(items: Sequence[Item], backends: Sequence[Any]) -> dict:
    out: dict[str, Any] = {"records": [], "missing": {}}
    live = list(backends)
    for it in items:
        p = it.payload
        eid = p["entity"].get("id") or p["entity"]["ext"][1]
        rec = {"id": it.id, "source": it.strata.get("source"), "relation": it.strata.get("relation"), "by_backend": {}}
        for b in list(live):
            try:
                t = b.entity(eid, limit=ENTITY_MAX_CLAIMS)
            except ComponentMissing as exc:
                out["missing"][b.name] = str(exc)
                live.remove(b)
                continue
            except BackendError as exc:
                rec["by_backend"][b.name] = {"ok": False, "problem": f"error: {exc}"}
                continue
            rec["by_backend"][b.name] = {**_check_provenance(t.value, p), "ms": t.ms}
        out["records"].append(rec)
    return out


def _check_provenance(resp: dict, p: dict) -> dict:
    claim = next((c for c in resp.get("claims", []) if c.get("id") == p["claim_id"]), None)
    if claim is None:  # absent from a truncated page = not served, so not a provenance defect; counted apart
        if resp.get("truncated"):
            return {"ok": None, "problem": "not_served_truncated"}
        return {"ok": False, "problem": "claim_missing"}
    provs = claim.get("provenance") or []
    if not provs or any(not pr.get(f) for pr in provs for f in PROV_FIELDS) or not claim.get("evidence_state"):
        return {"ok": False, "problem": "field_missing"}
    have = {(pr["source_id"], pr["locator"]) for pr in provs}
    if not all((e["source_id"], e["locator"]) in have for e in p["expected"]):
        return {"ok": False, "problem": "mismatch"}
    return {"ok": True, "problem": None}


def by_task(items: Iterable[Item]) -> dict[str, list[Item]]:
    out: dict[str, list[Item]] = {}
    for it in items:
        out.setdefault(it.task, []).append(it)
    return out
