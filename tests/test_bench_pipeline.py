"""AtlasBench pipeline: registry from a bundle, silver build, leakage, route checks, decision rules,
missing-component handling (no binary, unreachable or 501 HTTP, lexical fallback) and an end-to-end run.

Everything runs offline on tests/fixtures/mini_atlas.json; the HTTP tests use a loopback server.
"""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import Iterator

import pytest

from bench import evaluate, leakage, report, silver
from bench.__main__ import main
from bench.backends import ComponentMissing, HttpBackend, PathAnswer, StoreBackend
from bench.items import Item, read_jsonl
from bench.registry import Registry

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests" / "fixtures" / "mini_atlas.json"
ROUTES = ROOT / "bench" / "items" / "handwritten" / "routes.jsonl"


@pytest.fixture(scope="module")
def reg() -> Iterator[Registry]:
    r = Registry.open(FIXTURE)
    yield r
    r.close()


def _route_items(registry: str) -> list[Item]:
    return [i for i in read_jsonl(ROUTES) if i.strata.get("registry") == registry]


# ------------------------------------------------------------------ registry & silver

def test_bundle_registry_resolves_refs_and_has_events(reg: Registry) -> None:
    assert reg.ids({"id": "s:pythagoras"}) == ["s:pythagoras"]
    assert reg.ids({"id": "s:no-such"}) == []
    assert reg.event_seq() > 0
    assert reg.fingerprint()["path"].endswith("mini_atlas.json")


def test_silver_build_is_labeled_silver_and_route_labels_follow_the_oracle(reg: Registry) -> None:
    items, stats = silver.build(reg.store, seed=1)
    assert stats["items"] == len(items) > 0
    assert {i.origin for i in items} == {"silver"}
    assert {i.label_status for i in items} <= {"silver_auto", "unlabeled"}
    assert all(i.notes for i in items)  # every generator names its shared assumption
    assert silver.build(reg.store, seed=1)[0] == items  # seeded and deterministic
    for r in (i for i in items if i.task == "route"):
        assert r.payload["answerable"] == bool(r.payload["allowed"])
        assert not set(r.payload["allowed"]) & set(r.payload["forbidden"])


# ------------------------------------------------------------------ leakage

def test_leakage_flags_own_label_as_trivial_but_not_a_paraphrase(reg: Registry) -> None:
    e = reg.entity("s:pythagoras")
    own = e.labels[0].text
    assert leakage.check_qrel(own, e)["trivial"]
    assert leakage.check_qrel(own[:-2], e)["trivial"]  # last token works as a prefix, like Store.search
    para = leakage.check_qrel("zzqx wvut", e)
    assert not para["trivial"] and para["token_coverage"] == 0.0
    assert leakage.token_coverage("teorema zzqx", ["Teorema de Pitágoras"]) == 0.5


# ------------------------------------------------------------------ routes

def _step(cid: str, rel: str, a: str, b: str, direction: str = "forward", premises: list[str] | None = None) -> dict:
    return {"claim_id": cid, "relation": rel, "from": a, "to": b, "direction": direction,
            "premises": premises or [], "premises_missing": []}


def test_structural_check_catches_direction_staleness_and_missing_premises(reg: Registry) -> None:
    good = {"steps": [_step("c:cosines-gen", "generalizes", "s:law-of-cosines", "s:pythagoras"),
                      _step("c:pyth-surveying", "applied_in", "s:pythagoras", "app:surveying")]}
    assert evaluate.structural_problems(good, "s:law-of-cosines", "app:surveying", reg) == []
    backwards = {"steps": [_step("c:cosines-gen", "generalizes", "s:pythagoras", "s:law-of-cosines")]}
    assert any("direction" in p for p in
               evaluate.structural_problems(backwards, "s:pythagoras", "s:law-of-cosines", reg))
    conj = {"steps": [_step("c:pyth-euclid", "depends_on_logically", "s:pythagoras", "s:sas-congruence",
                            premises=["s:sas-congruence"])]}
    assert any("R7" in p for p in evaluate.structural_problems(conj, "s:pythagoras", "s:sas-congruence", reg))
    ghost = {"steps": [_step("c:nope", "generalizes", "s:law-of-cosines", "s:pythagoras")]}
    assert any("not in registry" in p for p in
               evaluate.structural_problems(ghost, "s:law-of-cosines", "s:pythagoras", reg))


def test_abstention_is_judged_against_answerability(reg: Registry) -> None:
    item = _route_items("mini_atlas")[0]
    j = evaluate.judge_path(item, PathAnswer(True, "no path"), "s:law-of-cosines", "app:surveying", reg)
    assert j["abstained"] and j["answerable"] == item.payload["answerable"]


def test_null_model_answers_every_connected_mini_atlas_route(reg: Registry) -> None:
    res = evaluate.routes(_route_items("mini_atlas"), reg, [StoreBackend(reg)])
    assert res["missing"] == {} and res["records"]
    answered = [r["by_backend"]["store"] for r in res["records"] if not r["by_backend"]["store"]["abstained"]]
    probs = [p for a in answered for p in a["structural_problems"]]
    # BFS steps are real, current claims; its only structural defect is hiding conjunctive premises (SPEC R7)
    assert answered and probs and all("R7" in p for p in probs)
    # the null model ignores composition: on adversarial items it must be caught claiming forbidden conclusions
    assert any(a.get("forbidden_violation") for a in answered)


# ------------------------------------------------------------------ decision rules (PROTOCOL §6)

def _ret_records(n: int, sys_ndcg: float, base_ndcg: float, cell: str = "es/text",
                 status: str = "expert_adjudicated", lanes: tuple[str, ...] = ("lexical", "semantic")) -> list[dict]:
    lang, mod = cell.split("/")
    return [{"id": f"q{i}", "origin": "expert", "label_status": status, "lang": lang, "modality": mod,
             "family": f"f{i}", "trivial": False,
             "by_backend": {"cli": {"ndcg@10": sys_ndcg, "lanes": list(lanes), "top": [f"x{i}"]},
                            "store": {"ndcg@10": base_ndcg, "lanes": ["lexical"], "top": [f"y{i}"]}}}
            for i in range(n)]


def test_retrieval_rule_pass_fail_insufficient_and_provisional() -> None:
    res = lambda recs: {"records": recs, "missing": {}, "unresolved": []}  # noqa: E731
    assert report.retrieval_verdict(res(_ret_records(30, 0.8, 0.5)), "cli")["verdict"] == "PASS"
    assert report.retrieval_verdict(res(_ret_records(30, 0.5, 0.8)), "cli")["verdict"] == "FAIL"
    assert report.retrieval_verdict(res(_ret_records(29, 0.8, 0.5)), "cli")["verdict"] == "INSUFFICIENT"
    agent = _ret_records(30, 0.8, 0.5, status="authored_by_agent_pending_expert_review")
    assert report.retrieval_verdict(res(agent), "cli")["verdict"] == "PROVISIONAL-PASS"


def test_lexical_only_system_is_missing_not_compared() -> None:
    recs = _ret_records(40, 0.5, 0.5, lanes=("lexical",))
    v = report.retrieval_verdict({"records": recs, "missing": {}, "unresolved": []}, "cli")
    assert v["verdict"] == "MISSING" and "lexical" in v["reason"]


def test_merge_rule_uses_wilson_and_straddle_is_insufficient() -> None:
    def res(k: int, n: int) -> dict:
        return {"records": [{"hardness": "system_merge", "label": "same" if i < k else "different",
                             "label_status": "expert_adjudicated", "type": "statement"} for i in range(n)]}
    assert report.er_verdict(res(381, 381))["verdict"] == "PASS"
    assert report.er_verdict(res(380, 380))["verdict"] == "INSUFFICIENT"  # n below the minimum
    assert report.er_verdict(res(399, 400))["verdict"] == "INSUFFICIENT"  # CI straddles 0.99
    assert report.er_verdict(res(380, 400))["verdict"] == "FAIL"


def test_adversarial_rule_needs_73_clean_items_to_pass() -> None:
    def res(n: int, bad: int) -> dict:
        recs = [{"label_status": "expert_adjudicated", "adversarial": True, "answerable": False, "attack": "x",
                 "by_backend": {"cli": {"abstained": False, "answerable": False, "structural_problems": [],
                                        "forbidden_violation": i < bad}}} for i in range(n)]
        return {"records": recs, "missing": {}, "unresolved": []}
    assert report.routes_verdicts(res(40, 0), "cli")["adversarial"]["verdict"] == "INSUFFICIENT"
    assert report.routes_verdicts(res(73, 0), "cli")["adversarial"]["verdict"] == "PASS"
    assert report.routes_verdicts(res(73, 1), "cli")["adversarial"]["verdict"] == "FAIL"
    assert report.routes_verdicts(res(39, 0), "cli")["adversarial"]["verdict"] == "INSUFFICIENT"


def test_silver_forbidden_list_binds_only_its_reference_path(reg: Registry) -> None:
    def item(scope: str) -> Item:
        return Item(id=f"t:route:{scope}", task="route", origin="silver", label_status="silver_auto", author="t",
                    created="2026-10-16", split_keys={"document": "d", "family": "f", "time": "2026-10-16"},
                    payload={"from": {"id": "a"}, "to": {"id": "c"}, "answerable": False, "allowed": [],
                             "forbidden": ["generalization"], "forbidden_scope": scope,
                             "steps": [{"from": {"id": "a"}, "to": {"id": "b"}, "relation": "generalizes",
                                        "direction": "forward", "label": "valid", "claim_id": "c:ref"}]})
    found = PathAnswer(False, None, ({"steps": [{"claim_id": "c:other", "relation": "generalizes", "from": "a",
                                                 "to": "c", "direction": "forward"}],
                                      "conclusion": {"kind": "generalization"}},))
    assert not evaluate.judge_path(item("reference_path"), found, "a", "c", reg)["forbidden_violation"]
    assert evaluate.judge_path(item("route_pair"), found, "a", "c", reg)["forbidden_violation"]


# ------------------------------------------------------------------ HTTP backend (loopback only)

class _Handler(BaseHTTPRequestHandler):
    mode = "501"

    def do_GET(self) -> None:  # noqa: N802 - http.server API
        if self.mode == "501" or (self.mode == "no_tiles" and self.path.startswith("/api/map/tiles")):
            code, body = 501, {"error": {"code": "not_implemented", "message": "atlas-search"}}
        else:
            code, body = 200, {"query": "q", "lang": "es", "limit": 10, "truncated": False,
                               "hits": [{"id": "s:pythagoras", "lanes": {"lexical": {"rank": 1, "score": 3.0},
                                                                         "formula": None, "semantic": None}}]}
        raw = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def log_message(self, *args: object) -> None:
        pass


@pytest.fixture
def server() -> Iterator[tuple[str, type[_Handler]]]:
    handler = type("H", (_Handler,), {})
    srv = HTTPServer(("127.0.0.1", 0), handler)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield f"http://127.0.0.1:{srv.server_address[1]}", handler
    srv.shutdown()
    srv.server_close()


def test_http_501_and_unreachable_are_missing_components(server: tuple[str, type[_Handler]]) -> None:
    url, _ = server
    with pytest.raises(ComponentMissing):
        HttpBackend(url).search("q", lang="es", limit=10)
    with pytest.raises(ComponentMissing):
        HttpBackend("http://127.0.0.1:9").search("q", lang="es", limit=10)


def test_http_lanes_are_parsed_per_hit(server: tuple[str, type[_Handler]]) -> None:
    url, handler = server
    handler.mode = "ok"
    got = HttpBackend(url).search("q", lang="es", limit=10).value
    assert got.ids == ("s:pythagoras",)
    assert got.lanes_used() == {"lexical"} and got.by_lane() == {"lexical": ["s:pythagoras"]}


def test_interaction_load_measures_each_endpoint_and_reports_501_as_missing(
        server: tuple[str, type[_Handler]]) -> None:
    from bench.__main__ import _interaction_load
    url, handler = server
    handler.mode = "no_tiles"
    lat: dict[str, list[float]] = {}
    notes: dict = {}
    _interaction_load(HttpBackend(url), ["s:pythagoras", "s:sas-congruence"], 7, lat, notes)
    assert len(lat["http.entity.load_c1"]) == 7 and len(lat["http.neighborhood.load_c1"]) == 7
    assert "http.tiles.load_c1" not in lat and "ComponentMissing" in notes["load_tiles"]


def test_rejected_query_scores_zero_instead_of_being_skipped(reg: Registry) -> None:
    from bench.backends import BackendError

    class Rejecting:
        name = "http"

        def search(self, *a: object, **k: object) -> None:
            raise BackendError("GET /api/search HTTP 400: bad_request: q too long")

    item = Item(id="t:ret:long", task="retrieval", origin="handwritten",
                label_status="authored_by_agent_pending_expert_review", author="test", created="2026-10-16",
                payload={"query": "x" * 300, "lang": "en", "modality": "text",
                         "qrels": [{"ref": {"id": "s:pythagoras"}, "grade": 3}]},
                strata={"area": "geometry"}, split_keys={"document": "t", "family": "s:pythagoras",
                                                         "time": "2026-10-16"})
    res = evaluate.retrieval([item], reg, [StoreBackend(reg), Rejecting()], set())
    got = res["records"][0]["by_backend"]["http"]
    assert got["ndcg@10"] == 0.0 and "failed" in got
    cell = report.retrieval_summary(res)["cells"]
    assert next(iter(cell.values()))["http"]["failed"] == 1


def test_claim_beyond_a_truncated_entity_page_is_not_a_provenance_failure() -> None:
    p = {"claim_id": "c:x", "expected": [{"source_id": "src:a", "locator": "l"}]}
    assert evaluate._check_provenance({"claims": [], "truncated": True}, p)["ok"] is None
    assert evaluate._check_provenance({"claims": [], "truncated": False}, p)["problem"] == "claim_missing"
    res = {"records": [{"by_backend": {"http": {"ok": None, "problem": "not_served_truncated"}}},
                       {"by_backend": {"http": {"ok": True, "problem": None}}}], "missing": {}}
    assert report.sourced_summary(res)["by_backend"]["http"]["not_served_truncated"] == 1
    assert report.sourced_verdict(res, "http")["failures"]["n"] == 1


# ------------------------------------------------------------------ end to end

def test_run_without_binary_reports_missing_and_still_evaluates_baselines(tmp_path: Path) -> None:
    out = tmp_path / "run"
    code = main(["run", "--db", str(FIXTURE), "--items", str(ROUTES), "--bin", str(tmp_path / "no-atlas.exe"),
                 "--out", str(out), "--load-n", "0"])
    assert code == 0
    rep = json.loads((out / "report.json").read_text(encoding="utf-8"))
    assert rep["backends"]["cli"].startswith("missing")
    assert rep["verdicts"]["routes.structure"]["verdict"] == "MISSING"
    assert rep["verdicts"]["retrieval"]["verdict"] in ("MISSING", "NOT_EVALUABLE")
    null = rep["summary"]["routes"]["by_backend"]["store"]
    assert null["structurally_valid"]["n"] > 0
    assert rep["items"]["route"] == len(read_jsonl(ROUTES))
    assert (out / "report.md").read_text(encoding="utf-8").startswith("# AtlasBench")


def test_null_graph_loader_matches_store_iter_edges(reg: Registry) -> None:
    from bench.backends import _Graph
    g = _Graph.build(reg)
    fast = {(s, t, cid, rel) for s, nb in g.adj.items() for t, cid, rel, d in nb if d == "forward"}
    ref = {(e.source, e.target, e.claim_id, e.relation.value) for e in reg.store.iter_edges()}
    assert fast == ref and ref
