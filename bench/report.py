"""Aggregation by stratum and the decision rules of PROTOCOL.md §6 (frozen 2026-10-16)."""

from __future__ import annotations

from collections import defaultdict
from typing import Any, Callable, Iterable, Sequence

from .metrics import bootstrap, min_n_for_lower_bound, paired_delta, percentiles, p95_ci, proportion

# ---- frozen thresholds (PROTOCOL.md §6); changing any of them requires protocol v2
MERGE_PRECISION_LB = 0.99
MERGE_STRATUM_MIN_N = 50
MERGE_MIN_N = min_n_for_lower_bound(MERGE_PRECISION_LB)  # 381
RELATION_PRECISION_LB = 0.97
RELATION_MIN_N = min_n_for_lower_bound(RELATION_PRECISION_LB)  # 125
PRIORITY_FAMILIES = ("proof", "structure", "application")
RETRIEVAL_CELL_MIN_N = 30
RETRIEVAL_MIN_DELTA = 0.05
RETRIEVAL_MIN_DELTA_LB = 0.02
ROUTE_ADVERSARIAL_MIN_N = 40
ROUTE_ADVERSARIAL_LB = 0.95
ROUTE_E2E_LB = 0.90
ROUTE_E2E_MIN_N = 100
ROUTE_MAX_ABSTAIN_ANSWERABLE = 0.25
ROUTE_UNANSWERABLE_ABSTAIN_LB = 0.90
PROVENANCE_MIN_N = 1000
SEARCH_P95_MS = 2000.0
INTERACTION_P95_MS = 100.0
LATENCY_MIN_N = 500
BASELINE = "store"
AREA_MIN_N = 10  # areas with fewer resolved queries are not tabulated separately (descriptive only, no gate)
GOLD = "expert_adjudicated"


def _verdict(level: str, gold: bool, rule: str, **detail: Any) -> dict:
    prefix = "" if gold or level in ("MISSING", "NOT_EVALUABLE", "INSUFFICIENT") else "PROVISIONAL-"
    return {"verdict": prefix + level, "rule": rule, **detail}


def _group(records: Iterable[dict], key: Callable[[dict], Any]) -> dict[Any, list[dict]]:
    out: dict[Any, list[dict]] = defaultdict(list)
    for r in records:
        out[key(r)].append(r)
    return dict(out)


# ------------------------------------------------------------------ retrieval

METRICS = ("ndcg@10", "recall@10", "recall@50", "mrr@10")


def retrieval_summary(res: dict) -> dict:
    recs = res["records"]
    backends = sorted({b for r in recs for b, v in r["by_backend"].items() if "error" not in v})
    out: dict[str, Any] = {"n_items": len(recs), "unresolved": len(res["unresolved"]), "missing": res["missing"],
                           "backends": backends, "cells": {}, "by_generator": {}}
    for name, rows in sorted(_group(recs, lambda r: f"{r['lang']}/{r['modality']}").items()):
        out["cells"][name] = _cell(rows, backends)
    for name, rows in sorted(_group(recs, lambda r: f"{r['origin']}:{r['generator']}"
                                                     f"{':trivial' if r['trivial'] else ''}").items()):
        out["by_generator"][name] = _cell(rows, backends)
    out["by_area"] = {a: _cell(rows, backends) for a, rows in sorted(_group(recs, lambda r: r["area"]).items())
                      if len(rows) >= AREA_MIN_N}
    out["qrels_resolved_share"] = proportion(sum(r["qrels_resolved"] for r in recs),
                                             sum(r["qrels_total"] for r in recs))
    out["lanes_used"] = {b: sorted({ln for r in recs for ln in r["by_backend"].get(b, {}).get("lanes", [])})
                         for b in backends}
    return out


def _cell(rows: list[dict], backends: Sequence[str]) -> dict:
    cell: dict[str, Any] = {"n": len(rows), "trivial": sum(r["trivial"] for r in rows)}
    for b in backends:
        ok = [r for r in rows if b in r["by_backend"] and "error" not in r["by_backend"][b]]
        fams = [r["family"] for r in ok]
        cell[b] = {m: bootstrap([r["by_backend"][b][m] for r in ok], clusters=fams) for m in METRICS}
        cell[b]["failed"] = sum("failed" in r["by_backend"][b] for r in ok)
        if b != BASELINE and BASELINE in backends:
            both = [r for r in ok if BASELINE in r["by_backend"] and "error" not in r["by_backend"][BASELINE]]
            lanes = sorted({ln for r in ok for ln in r["by_backend"][b].get("by_lane", {})})
            cell[b]["by_lane"] = {ln: {m: bootstrap([r["by_backend"][b]["by_lane"].get(ln, {}).get(m, 0.0)
                                                     for r in ok], clusters=fams) for m in ("ndcg@10", "recall@10")}
                                  for ln in lanes}
            cell[b]["delta_ndcg@10_vs_baseline"] = paired_delta(
                [r["by_backend"][b]["ndcg@10"] for r in both], [r["by_backend"][BASELINE]["ndcg@10"] for r in both],
                clusters=[r["family"] for r in both])
    return cell


def retrieval_verdict(res: dict, system: str) -> dict:
    rule = ("each lang x modality cell with n >= 30 (trivial silver excluded): delta nDCG@10 >= +0.05 and "
            "paired-bootstrap LB >= +0.02 vs lexical baseline")
    if system in res["missing"]:
        return _verdict("MISSING", False, rule, reason=res["missing"][system])
    answered = [r for r in res["records"] if "error" not in r["by_backend"].get(system, {"error": 1})]
    lanes = {ln for r in answered for ln in r["by_backend"][system]["lanes"]}
    if answered and lanes <= {"lexical"}:  # PROTOCOL §6.1 (a): the fallback is the baseline, not a candidate
        both = [r for r in answered if "error" not in r["by_backend"].get(BASELINE, {"error": 1})]
        same = sum(r["by_backend"][system]["top"] == r["by_backend"][BASELINE]["top"] for r in both)
        return _verdict("MISSING", False, rule, reason="hybrid search absent: the system answered every query with "
                        "the lexical lane only (registry FTS5 fallback = the baseline)",
                        top10_identical_to_baseline=proportion(same, len(both)))
    gating = [r for r in res["records"] if not (r["trivial"] and r["origin"] == "silver")
              and "error" not in r["by_backend"].get(system, {"error": 1})
              and "error" not in r["by_backend"].get(BASELINE, {"error": 1})]
    if not gating:
        return _verdict("NOT_EVALUABLE", False, rule, reason="no non-trivial resolved items for system and baseline")
    gold = all(r["label_status"] == GOLD for r in gating)
    cells, failing, insufficient = {}, [], []
    for name, rows in sorted(_group(gating, lambda r: f"{r['lang']}/{r['modality']}").items()):
        d = paired_delta([r["by_backend"][system]["ndcg@10"] for r in rows],
                         [r["by_backend"][BASELINE]["ndcg@10"] for r in rows], clusters=[r["family"] for r in rows])
        cells[name] = d
        if len(rows) < RETRIEVAL_CELL_MIN_N:
            insufficient.append(name)
        elif d["ci95"][1] < 0 or d["value"] < RETRIEVAL_MIN_DELTA or d["ci95"][0] < RETRIEVAL_MIN_DELTA_LB:
            failing.append(name)
    level = "FAIL" if failing else ("INSUFFICIENT" if len(insufficient) == len(cells) else "PASS")
    return _verdict(level, gold, rule, cells=cells, failing=failing, insufficient=insufficient)


# ------------------------------------------------------------------ routes

def routes_summary(res: dict) -> dict:
    out: dict[str, Any] = {"n_items": len(res["records"]), "unresolved": len(res["unresolved"]),
                           "missing": res["missing"], "by_backend": {}}
    backends = sorted({b for r in res["records"] for b in r["by_backend"]})
    for b in backends:
        rows = [r["by_backend"][b] | {"rec": r} for r in res["records"] if b in r["by_backend"]]
        ok = [x for x in rows if "error" not in x]
        answered = [x for x in ok if not x["abstained"]]
        adv = [x for x in ok if x["rec"]["adversarial"]]
        ans_items = [x for x in ok if x["answerable"]]
        out["by_backend"][b] = {
            "errors": len(rows) - len(ok),
            "structurally_valid": proportion(sum(not x["structural_problems"] for x in answered), len(answered)),
            "adversarial_no_forbidden": proportion(sum(not x.get("forbidden_violation") for x in adv), len(adv)),
            "answerable_allowed_hit": proportion(sum(bool(x.get("allowed_hit")) for x in ans_items), len(ans_items)),
            "abstain_on_answerable": proportion(sum(x["abstained"] for x in ans_items), len(ans_items)),
            "abstain_on_unanswerable": proportion(sum(x["abstained"] for x in ok if not x["answerable"]),
                                                  sum(not x["answerable"] for x in ok)),
            "overclaim_vs_oracle": proportion(sum(bool(x.get("overclaim")) for x in answered), len(answered)),
            "by_attack": {k: proportion(sum(not x.get("forbidden_violation") for x in v), len(v))
                          for k, v in sorted(_group(adv, lambda x: x["rec"]["attack"] or "none").items())},
            "example_problems": [p for x in answered for p in x["structural_problems"]][:5]}
    return out


def routes_verdicts(res: dict, system: str) -> dict:
    if system in res["missing"]:
        reason = res["missing"][system]
        return {k: _verdict("MISSING", False, k, reason=reason) for k in ("structure", "adversarial", "end_to_end")}
    rows = [r["by_backend"][system] | {"rec": r} for r in res["records"]
            if system in r["by_backend"] and "error" not in r["by_backend"][system]]
    if not rows:
        return {k: _verdict("NOT_EVALUABLE", False, k, reason="no resolved route items")
                for k in ("structure", "adversarial", "end_to_end")}
    gold = all(x["rec"]["label_status"] == GOLD for x in rows)
    answered = [x for x in rows if not x["abstained"]]
    bad = sum(bool(x["structural_problems"]) for x in answered)
    structure = _verdict("FAIL" if bad else "PASS", True, "0 structurally invalid steps in returned paths",
                         invalid_paths=bad, answered=len(answered))
    adv = [x for x in rows if x["rec"]["adversarial"]]
    k = sum(not x.get("forbidden_violation") for x in adv)
    prop = proportion(k, len(adv))
    # PROTOCOL §6.1 (g): any forbidden conclusion is a defect; no violations but LB < 0.95 (n < 73) is INSUFFICIENT
    lvl = ("INSUFFICIENT" if len(adv) < ROUTE_ADVERSARIAL_MIN_N else "FAIL" if k < len(adv) else
           "PASS" if prop["ci95"][0] >= ROUTE_ADVERSARIAL_LB else "INSUFFICIENT")
    adversarial = _verdict(lvl, gold, "no forbidden conclusion on adversarial items, Wilson LB >= 0.95, n >= 40",
                           no_forbidden=prop)
    end_to_end = _verdict("NOT_EVALUABLE", False, "Wilson LB > 0.90 of expert-valid top-1 routes (n >= 100)",
                          reason="no expert judgments of returned routes yet; route_outputs.jsonl is the queue")
    return {"structure": structure, "adversarial": adversarial, "end_to_end": end_to_end}


# ------------------------------------------------------------------ entity resolution, relations, provenance

def er_summary(res: dict) -> dict:
    recs = res["records"]
    out: dict[str, Any] = {"n_items": len(recs), "unresolved": len(res["unresolved"]),
                           "abstention": proportion(sum(r["pred"] == "abstain" for r in recs), len(recs))}
    out["false_merge_rate_on_negatives"] = {
        h: proportion(sum(r["pred"] == "same" for r in rows), len(rows))
        for h, rows in sorted(_group([r for r in recs if r["label"] == "different"], lambda r: r["hardness"]).items())}
    out["merge_recall_on_positives"] = {
        f"{h}{' (circular)' if rows[0]['circular'] else ''}": proportion(sum(r["pred"] == "same" for r in rows),
                                                                        len(rows))
        for h, rows in sorted(_group([r for r in recs if r["label"] == "same"], lambda r: r["hardness"]).items())}
    sm = [r for r in recs if r["hardness"] == "system_merge"]
    out["system_merges_sampled"] = len(sm)
    out["system_merges_labeled"] = sum(r["label"] is not None for r in sm)
    return out


def er_verdict(res: dict) -> dict:
    rule = "Wilson LB of merge precision >= 0.99 on sampled system merges (n >= 381); strata n >= 50 point >= 0.99"
    sm = [r for r in res["records"] if r["hardness"] == "system_merge" and r["label"] is not None]
    if not sm:
        return _verdict("NOT_EVALUABLE", False, rule, reason="no labeled sample of system merges (needs experts)")
    gold = all(r["label_status"] == GOLD for r in sm)
    prec = proportion(sum(r["label"] == "same" for r in sm), len(sm))
    strata = {k: proportion(sum(r["label"] == "same" for r in v), len(v))
              for k, v in _group(sm, lambda r: r["type"]).items()}
    big = {k: v for k, v in strata.items() if v["n"] >= MERGE_STRATUM_MIN_N}
    if len(sm) < MERGE_MIN_N:
        return _verdict("INSUFFICIENT", gold, rule, precision=prec, strata=strata)
    fail = prec["ci95"][1] < MERGE_PRECISION_LB or any(v["ci95"][1] < MERGE_PRECISION_LB for v in big.values())
    ok = prec["ci95"][0] >= MERGE_PRECISION_LB and all(v["value"] >= MERGE_PRECISION_LB for v in big.values())
    lvl = "FAIL" if fail else "PASS" if ok else "INSUFFICIENT"  # PROTOCOL §6.1 (g): CI straddles the target
    return _verdict(lvl, gold, rule, precision=prec, strata=strata)


def relations_summary(res: dict) -> dict:
    recs = res["records"]
    return {"n_items": len(recs), "unresolved": len(res["unresolved"]),
            "labeled": sum(r["label"] is not None for r in recs),
            "still_current": proportion(sum(r["current"] for r in recs), len(recs)),
            "corroborated_by_another_source": {
                f: proportion(sum(r["corroborated"] for r in v), len(v))
                for f, v in sorted(_group(recs, lambda r: r["family"]).items())},
            "by_relation": {k: len(v) for k, v in sorted(_group(recs, lambda r: r["relation"]).items())}}


def relations_verdict(res: dict) -> dict:
    rule = "each priority family (proof, structure, application): Wilson LB of precision >= 0.97, n >= 125"
    lab = [r for r in res["records"] if r["label"] is not None]
    if not lab:
        return _verdict("NOT_EVALUABLE", False, rule, reason="no labeled relation items (needs experts)")
    gold = all(r["label_status"] == GOLD for r in lab)
    fams = {f: proportion(sum(r["label"] == "correct" for r in v), len(v))
            for f, v in _group(lab, lambda r: r["family"]).items()}
    pri = {f: fams.get(f, proportion(0, 0)) for f in PRIORITY_FAMILIES}
    if any(v["n"] < RELATION_MIN_N for v in pri.values()):
        return _verdict("INSUFFICIENT", gold, rule, families=fams)
    ok = all(v["ci95"][0] >= RELATION_PRECISION_LB for v in pri.values())
    fail = any(v["ci95"][1] < RELATION_PRECISION_LB for v in pri.values())
    return _verdict("FAIL" if fail else "PASS" if ok else "INSUFFICIENT", gold, rule, families=fams)


def sourced_summary(res: dict) -> dict:
    out: dict[str, Any] = {"n_items": len(res["records"]), "missing": res["missing"], "by_backend": {}}
    for b in sorted({b for r in res["records"] for b in r["by_backend"]}):
        every = [r["by_backend"][b] for r in res["records"] if b in r["by_backend"]]
        rows = [x for x in every if x["ok"] is not None]  # ok=None: claim beyond a truncated page, not served
        probs = _group([x for x in every if not x["ok"]], lambda x: x["problem"])
        fails = len(rows) - sum(x["ok"] for x in rows)
        out["by_backend"][b] = {"complete_and_matching": proportion(len(rows) - fails, len(rows)),
                                "failure_rate": proportion(fails, len(rows)),
                                "not_served_truncated": len(every) - len(rows),
                                "problems": {k: len(v) for k, v in probs.items()}}
    return out


def sourced_verdict(res: dict, backend: str) -> dict:
    rule = "0 served claims without complete, matching provenance; n >= 1000 (mechanical check)"
    if backend in res["missing"]:
        return _verdict("MISSING", False, rule, reason=res["missing"][backend])
    rows = [r["by_backend"][backend] for r in res["records"]
            if backend in r["by_backend"] and r["by_backend"][backend]["ok"] is not None]
    if not rows:
        return _verdict("NOT_EVALUABLE", False, rule, reason="no items")
    fails = sum(not x["ok"] for x in rows)
    lvl = "FAIL" if fails else ("INSUFFICIENT" if len(rows) < PROVENANCE_MIN_N else "PASS")
    return _verdict(lvl, True, rule, failures=proportion(fails, len(rows)))


def latency_summary(samples: dict[str, list[float]]) -> dict:
    return {k: {**percentiles(v), "p95_ci": p95_ci(v)["ci95"] if v else None} for k, v in sorted(samples.items())}


def latency_verdict(samples: list[float], threshold_ms: float, label: str) -> dict:
    rule = f"bootstrap 95% upper bound of p95 < {threshold_ms:.0f} ms, n >= {LATENCY_MIN_N}"
    if not samples:
        return _verdict("NOT_EVALUABLE", True, rule, reason=f"no {label} samples")
    ci = p95_ci(samples)
    lvl = ("INSUFFICIENT" if len(samples) < LATENCY_MIN_N else
           "PASS" if ci["ci95"][1] < threshold_ms else "FAIL")
    return _verdict(lvl, True, rule, p95=ci)
