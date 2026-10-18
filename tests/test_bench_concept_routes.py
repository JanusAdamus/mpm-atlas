"""bench.concept_routes: a concept route is judged on its own steps, witnesses and memberships."""

from types import SimpleNamespace as NS

from bench.concept_routes import judge, structural_problems
from bench.items import Item


def _claim(cid: str, rel: str, subject: str, targets: tuple[str, ...]) -> NS:
    return NS(id=cid, relation=NS(value=rel), subject=subject, targets=targets, evidence_state=NS(value="extracted"))


CLAIMS = {c.id: c for c in (
    _claim("c:dep", "depends_on_logically", "d:a1", ("d:b1",)),
    _claim("c:ma", "member_of_concept", "d:a1", ("k:A",)),
    _claim("c:mb", "member_of_concept", "d:b1", ("k:B",)),
)}
REG = NS(claim=CLAIMS.get, is_current=lambda cid: True, canonical=lambda x: x,
         store=NS(claims_for=lambda eid, relations=None: []))


def _level(direction: str = "forward", witness_from: str = "d:a1", kind: str = "logical_dependency") -> dict:
    mems = [{"declaration": "d:a1", "concept": "k:A", "claim_id": "c:ma"},
            {"declaration": "d:b1", "concept": "k:B", "claim_id": "c:mb"}]
    step = {"from": "k:A", "to": "k:B", "relation": "depends_on_logically", "direction": direction,
            "witness": {"claim_id": "c:dep", "from": witness_from, "to": "d:b1", "sources": [{"source_id": "s"}],
                        "memberships": mems}}
    return {"abstained": False, "from": {"requested": "k:A", "concept": "k:A", "via": "concept"},
            "to": {"requested": "k:B", "concept": "k:B", "via": "concept"},
            "route": {"conclusion": {"kind": kind, "is_hypothesis": False}, "steps": [step]}}


def _item(allowed: list[str], forbidden: list[str]) -> Item:
    return Item("hw:concept-route:t", "route", "handwritten", "authored_by_agent_pending_expert_review", "t",
                "2026-10-18", {"level": "concept", "from": {"id": "k:A"}, "to": {"id": "k:B"}, "answerable": bool(allowed),
                               "attack": None, "allowed": allowed, "forbidden": forbidden},
                split_keys={"document": "d", "family": "f", "time": "2026-10-18"})


def test_valid_concept_route_passes_and_matches_the_oracle() -> None:
    r = judge(_item(["logical_dependency"], ["logical_support"]), {"concepts": _level()}, REG)
    assert r["structural_problems"] == [] and r["allowed_hit"] and not r["overclaim"]


def test_witness_walked_the_wrong_way_or_without_membership_is_flagged() -> None:
    assert any("direction" in p for p in structural_problems(_level(direction="inverse"), REG))
    assert any("membership" in p for p in structural_problems(_level(witness_from="d:other"), REG))


def test_conclusion_stronger_than_the_steps_is_an_overclaim() -> None:
    lvl = _level()
    lvl["route"]["steps"][0]["relation"] = "cites"  # bibliographic step, logical conclusion
    r = judge(_item(["logical_dependency"], []), {"concepts": lvl}, REG)
    assert r["overclaim"]


def test_abstention_is_recorded_not_judged() -> None:
    r = judge(_item([], ["logical_dependency"]), {"concepts": {"abstained": True, "abstention": "no_route"}}, REG)
    assert r["abstained"] and "forbidden_violation" not in r
