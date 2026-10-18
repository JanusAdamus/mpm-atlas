"""AtlasBench units: intervals, ranking metrics, agreement, composition oracle, item format, splits."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from bench import compose, metrics, splits
from bench.compose import Hop
from bench.items import Item, ItemError, item_hash, read_jsonl, write_jsonl

ROOT = Path(__file__).resolve().parents[1]
HANDWRITTEN = ROOT / "bench" / "items" / "handwritten"


# ------------------------------------------------------------------ intervals

def test_wilson_matches_closed_form_values() -> None:
    lo, hi = metrics.wilson(5, 10)
    assert lo == pytest.approx(0.2366, abs=1e-4) and hi == pytest.approx(0.7634, abs=1e-4)
    assert metrics.wilson(0, 0) == (0.0, 1.0)
    assert metrics.wilson(10, 10)[1] == 1.0


def test_min_n_for_zero_failure_lower_bound() -> None:
    assert metrics.min_n_for_lower_bound(0.99) == 381
    assert metrics.min_n_for_lower_bound(0.97) == 125
    assert metrics.wilson(381, 381)[0] >= 0.99 > metrics.wilson(380, 380)[0]


def test_bootstrap_is_seeded_and_brackets_the_mean() -> None:
    x = [0.0, 1.0] * 50
    a, b = metrics.bootstrap(x), metrics.bootstrap(x)
    assert a == b
    assert a["ci95"][0] < 0.5 < a["ci95"][1]
    clustered = metrics.bootstrap(x, clusters=[i // 10 for i in range(100)])
    assert clustered["clusters"] == 10


def test_paired_delta_of_identical_systems_is_zero() -> None:
    d = metrics.paired_delta([0.2, 0.5, 0.9], [0.2, 0.5, 0.9])
    assert d["value"] == 0.0 and d["ci95"] == [0.0, 0.0]


# ------------------------------------------------------------------ ranking

def test_graded_ndcg_recall_mrr() -> None:
    grades = {"a": 3, "b": 1}
    # dcg = 1/1 + 7/log2(3) ; ideal = 7/1 + 1/log2(3)
    assert metrics.ndcg_at_k(["b", "a"], grades) == pytest.approx(0.709810, abs=1e-5)
    assert metrics.ndcg_at_k(["a", "b"], grades) == pytest.approx(1.0)
    assert metrics.ndcg_at_k(["a", "a", "b"], grades) == pytest.approx(1.0)  # duplicates do not count twice
    assert metrics.recall_at_k(["x", "a"], grades, k=1) == 0.0
    assert metrics.recall_at_k(["x", "a"], grades, k=2) == 1.0
    assert metrics.mrr_at_k(["x", "b", "a"], grades) == pytest.approx(1 / 3)
    with pytest.raises(ValueError):
        metrics.ndcg_at_k(["a"], {"a": 0})


def test_latency_percentiles() -> None:
    p = metrics.percentiles(list(range(1, 101)))
    assert p["n"] == 100 and p["p50"] == pytest.approx(50.5) and p["p95"] == pytest.approx(95.05)


# ------------------------------------------------------------------ agreement

def test_cohen_kappa_hand_computed() -> None:
    # po = 0.75, pe = 0.5*0.25 + 0.5*0.75 = 0.5  ->  kappa = 0.5
    assert metrics.cohen_kappa([1, 1, 0, 0], [1, 0, 0, 0]) == pytest.approx(0.5)
    assert metrics.cohen_kappa(["a", "b"], ["a", "b"]) == 1.0


def test_krippendorff_alpha_reference_example() -> None:
    # Krippendorff (2011), "Computing Krippendorff's alpha-reliability": 4 coders x 12 units with gaps.
    rows = {"A": [1, 2, 3, 3, 2, 1, 4, 1, 2, None, None, None],
            "B": [1, 2, 3, 3, 2, 2, 4, 1, 2, 5, None, 3],
            "C": [None, 3, 3, 3, 2, 3, 4, 2, 2, 5, 1, None],
            "D": [1, 2, 3, 3, 2, 4, 4, 1, 2, 5, 1, None]}
    units = {u: {r: rows[r][u] for r in rows} for u in range(12)}
    assert metrics.krippendorff_alpha(units, "nominal") == pytest.approx(0.743, abs=5e-4)
    assert metrics.krippendorff_alpha(units, "interval") == pytest.approx(0.849, abs=5e-4)


# ------------------------------------------------------------------ composition oracle

@pytest.mark.parametrize("hops,expected", [
    ([Hop("generalizes"), Hop("applied_in")], "application_via_special_case"),
    ([Hop("generalizes"), Hop("generalizes"), Hop("applied_in")], "application_via_special_case"),
    ([Hop("special_case_of"), Hop("applied_in")], "none"),
    ([Hop("analogous_to"), Hop("generalizes")], "hypothesis"),
    ([Hop("part_of_area"), Hop("part_of_area")], "area_membership"),
    ([Hop("generalizes", "inverse")], "specialization"),
    ([Hop("depends_on_logically", "inverse")], "logical_support"),
    ([Hop("prerequisite_for"), Hop("depends_on_logically", "inverse")], "study_route"),
    ([Hop("named_after"), Hop("named_after", "inverse")], "historical"),
    ([Hop("cites"), Hop("cites")], "bibliographic"),
    ([Hop("appears_in"), Hop("authored_by")], "documentary"),
    ([Hop("equivalent_to"), Hop("generalizes")], "generalization"),
    ([Hop("applied_in"), Hop("applied_in")], "none"),
    ([Hop("depends_on_logically"), Hop("depends_on_logically")], "logical_dependency"),
])
def test_oracle_follows_spec_rules(hops: list[Hop], expected: str) -> None:
    assert compose.conclude(hops) == expected


def test_oracle_reproduces_every_expectation_and_pair_of_the_table() -> None:
    t = compose.table()
    assert t["expect"], "composition.json has no expectations to check against"
    for e in t["expect"]:
        assert compose.conclude(compose.parse_path(e["path"])) == e["conclusion"], e
    for p in t["pairs"]:
        assert compose.conclude([Hop(p["first"]), Hop(p["then"])]) == p["result"], p


def test_conclusion_text_normalization() -> None:
    assert compose.normalize_conclusion("a study order suggestion; never a logical necessity") == "study_route"
    assert compose.normalize_conclusion("study order suggestion") == "study_route"
    assert compose.normalize_conclusion("Generalización") == "generalization"
    assert compose.normalize_conclusion("anything", is_hypothesis=True) == "hypothesis"
    assert compose.normalize_conclusion("zzz") == "unparsed"


# ------------------------------------------------------------------ item format

def _ret(iid: str, fam: str = "f", time: str = "2026-01-01", origin: str = "handwritten",
         status: str = "authored_by_agent_pending_expert_review", doc: str | None = None) -> Item:
    return Item(iid, "retrieval", origin, status, "t", "2026-10-16",
                {"query": "q", "lang": "es", "modality": "text", "qrels": [{"ref": {"id": "x"}, "grade": 3}]},
                {}, {"document": doc or f"d:{iid}", "family": fam, "time": time})


def test_items_validate_and_roundtrip(tmp_path: Path) -> None:
    it = _ret("a")
    assert Item.from_json(json.loads(json.dumps(it.to_json()))) == it
    write_jsonl(tmp_path / "x.jsonl", [it])
    assert item_hash(read_jsonl(tmp_path / "x.jsonl")[0]) == item_hash(it)
    with pytest.raises(ItemError):
        _ret("b", origin="silver", status="expert_adjudicated")  # silver can never claim expert labels
    with pytest.raises(ItemError):
        Item("c", "retrieval", "handwritten", "unlabeled", "t", "2026-10-16",
             {"query": "q", "lang": "es", "modality": "text", "qrels": [{"ref": {"id": "x"}, "grade": 1}]},
             {}, {"document": "d", "family": "f", "time": "2026-01-01"})
    with pytest.raises(ItemError):
        Item("d", "route", "handwritten", "unlabeled", "t", "2026-10-16",
             {"from": {"id": "a"}, "to": {"id": "b"}, "answerable": True, "allowed": ["x"], "forbidden": ["x"]},
             {}, {"document": "d", "family": "f", "time": "2026-01-01"})


def test_handwritten_seed_meets_the_brief() -> None:
    ret = read_jsonl(HANDWRITTEN / "retrieval.jsonl")
    routes = read_jsonl(HANDWRITTEN / "routes.jsonl")
    assert len(ret) >= 60 and len(routes) >= 40
    assert {i.payload["lang"] for i in ret} == {"es", "en"}
    mods = {i.payload["modality"] for i in ret}
    assert {"text", "formula", "lean_name"} <= mods
    assert sum(1 for i in routes if not i.payload["answerable"] or i.payload["attack"]) >= 15
    for i in ret + routes:
        assert i.origin == "handwritten" and i.label_status == "authored_by_agent_pending_expert_review"
    for r in routes:
        for k in r.payload["allowed"] + r.payload["forbidden"]:
            assert k in compose.table()["conclusions"], (r.id, k)


# ------------------------------------------------------------------ splits

def test_splits_group_by_document_and_family_and_respect_time() -> None:
    items = [_ret(f"i{n}", fam=f"fam{n % 7}", doc=f"doc{n % 11}") for n in range(60)]
    items += [_ret("late", fam="late-fam", time="2027-01-01", doc="late-doc")]
    out = splits.assign(items, cutoff="2026-10-16")
    assert splits.leaks(out) == []
    assert sum(len(v) for v in out.values()) == len(items)
    assert all(i.id != "late" for i in out["dev"])
    assert splits.assign(items, cutoff="2026-10-16") == out  # deterministic


def test_manifest_detects_edits_and_sealed_ledger_counts(tmp_path: Path) -> None:
    items = [_ret(f"i{n}", fam=f"fam{n}") for n in range(30)]
    man = splits.write_splits(items, tmp_path / "s", tmp_path / "m.json", version="t", cutoff="2026-10-16")
    assert sum(v["count"] for v in man["splits"].values()) == 30
    assert splits.verify(tmp_path / "m.json", tmp_path) == []
    target = next(n for n in splits.SPLITS if man["splits"][n]["count"])
    path = tmp_path / "s" / f"{target}.jsonl"
    first = read_jsonl(path)
    changed = [Item.from_json({**first[0].to_json(), "notes": "edited"})] + first[1:]
    write_jsonl(path, changed)
    assert any("changed" in p for p in splits.verify(tmp_path / "m.json", tmp_path))
    assert splits.sealed_openings(tmp_path) == 0
    assert splits.log_sealed_opening(tmp_path, reason="release v1", manifest_version="t", system="x") == 1
    with pytest.raises(ValueError):
        splits.log_sealed_opening(tmp_path, reason=" ", manifest_version="t", system="x")
