"""Entity resolution: blocking, scoring, thresholds, reversible merges, evaluation."""

from __future__ import annotations

import json
import time

import pytest

from atlas import Entity, EntityType, EvidenceState, LangString, Provenance, RelationType, Source, Store, binary
from atlas import resolve

TS = "2026-10-16T00:00:00+00:00"
SRC = Source("src:test", "resolve test data", "CC0-1.0")
PROV = (Provenance("src:test", "test", "hand", "1", "CC0-1.0", TS),)


def decl(name: str, etype: EntityType = EntityType.STATEMENT, doc: str | None = None,
         alias_of: str | None = None) -> Entity:
    ext = (("mathlib", name),) + ((("mathlib_alias_of", alias_of),) if alias_of else ())
    return Entity("mathlib:" + name, etype, (LangString(name, "und"),),
                  descriptions=(LangString(doc, "en"),) if doc else (), external_ids=ext, provenance=PROV)


def item(qid: str, label: str, etype: EntityType = EntityType.CONCEPT, aliases: tuple[str, ...] = ()) -> Entity:
    return Entity("wd:" + qid, etype, (LangString(label, "en"), LangString(label + " (es)", "es")),
                  aliases=tuple(LangString(a, "en") for a in aliases), external_ids=(("wikidata", qid),),
                  provenance=PROV)


@pytest.fixture()
def store():
    s = Store()
    s.add_source(SRC)
    s.add_entities([
        decl("Nat.Prime", EntityType.DEFINITION, "`Nat.Prime p` means that `p` is a prime number."),
        decl("CauchySeq", EntityType.DEFINITION, "A Cauchy sequence."),
        decl("EuclideanGeometry.dist_sq_eq_dist_sq_add_dist_sq_iff_angle_eq_pi_div_two", EntityType.STATEMENT,
             "**Pythagorean theorem**: the square of the hypotenuse equals the sum of the squares."),
        decl("add_pow", EntityType.STATEMENT, "The **binomial theorem**"),
        decl("Nat.Prime.two_le", EntityType.STATEMENT),
        decl("Nat.prime_two_le", EntityType.STATEMENT, alias_of="Nat.Prime.two_le"),
        decl("Unrelated.thing", EntityType.STATEMENT, "Nothing to see."),
        item("Q49008", "prime number", aliases=("prime",)),
        item("Q11518", "Pythagorean theorem", EntityType.STATEMENT),
        item("Q26708", "binomial theorem", EntityType.STATEMENT),
        item("Q1052803", "Cauchy sequence"),
        item("Q999", "monster group"),
        Entity("wd:Q12479", EntityType.AREA, (LangString("number theory", "en"),), provenance=PROV),
        Entity("mathlib-dir:Mathlib/NumberTheory", EntityType.AREA, (LangString("Mathlib/NumberTheory", "und"),),
               provenance=PROV),
    ])
    yield s
    s.close()


def test_normalisation_and_tokens() -> None:
    assert resolve.normalize("Pythagoras' théorème, n°1") == "pythagoras theoreme n 1"
    assert resolve.content_tokens("Fermat's Little Theorem") == ("fermat", "little", "theorem")
    assert resolve.decl_last_tokens("Real.CauchySeq") == ("cauchy", "sequence")


def test_alias_merge_is_recorded_and_undoable(store: Store) -> None:
    cands = list(resolve.alias_candidates(store))
    assert [(c.left, c.right, c.score) for c in cands] == [("mathlib:Nat.Prime.two_le", "mathlib:Nat.prime_two_le",
                                                            1.0)]
    report = __import__("collections").Counter()
    resolve.apply_merges(store, cands, report)
    assert report["merges_applied"] == 1
    assert store.resolve("mathlib:Nat.prime_two_le") == "mathlib:Nat.Prime.two_le"
    (m,) = store.merges(active_only=True)
    ev = json.loads(m.evidence)
    assert m.actor == resolve.ACTOR and ev["resolver"] == resolve.RESOLVER_VERSION and ev["features"]["explicit_alias"]
    resolve.apply_merges(store, cands, report)  # idempotent
    assert report["identity_already_merged"] == 1
    assert resolve.undo_resolver_merges(store, actor="tester", reason="check reversibility") == 1
    assert store.resolve("mathlib:Nat.prime_two_le") == "mathlib:Nat.prime_two_le"


def test_merge_requires_threshold_and_same_type(store: Store) -> None:
    from collections import Counter

    rep: Counter = Counter()
    low = resolve.Candidate("mathlib:Nat.Prime.two_le", "mathlib:Unrelated.thing", 0.9, "identity", ())
    cross = resolve.Candidate("mathlib:Nat.Prime", "mathlib:Unrelated.thing", 1.0, "identity", ())
    resolve.apply_merges(store, [low, cross], rep)
    assert rep["identity_below_threshold"] == 1 and rep["identity_type_mismatch"] == 1
    assert store.merges() == []


def test_formalization_candidates_use_docstrings_and_names(store: Store) -> None:
    idx = resolve.MathlibIndex.from_store(store)
    assert "mathlib:Nat.prime_two_le" not in {r.id for r in idx.rows}  # aliases are not targets
    by_item: dict[str, list] = {}
    for c in resolve.formalization_candidates(store, idx):
        by_item.setdefault(c.left, []).append(c)
    assert by_item["wd:Q11518"][0].right.startswith("mathlib:EuclideanGeometry.dist_sq")
    assert dict(by_item["wd:Q11518"][0].features)["bold"] is True
    assert by_item["wd:Q26708"][0].right == "mathlib:add_pow"
    assert by_item["wd:Q1052803"][0].right == "mathlib:CauchySeq"  # seq -> sequence
    assert "wd:Q999" not in by_item  # no evidence, no candidate


def test_links_above_threshold_become_model_proposed_claims(store: Store) -> None:
    from collections import Counter

    rep: Counter = Counter()
    queue: list[dict] = []
    cands = list(resolve.formalization_candidates(store))
    resolve.apply_links(store, cands, rep, queue)
    claims = list(store.iter_claims(RelationType.FORMALIZED_AS))
    assert claims and all(c.evidence_state is EvidenceState.MODEL_PROPOSED for c in claims)
    for c in claims:
        assert c.confidence.value >= resolve.LINK_THRESHOLD
        assert "not calibrated" in c.confidence.calibration and c.confidence.estimates == resolve.ESTIMATES
        assert c.provenance[0].source_id == resolve.SOURCE_ID
    assert all(row["score"] < resolve.LINK_THRESHOLD or row["reason"] == "not_top_ranked" for row in queue)
    assert len({c.subject for c in claims}) == len(claims)  # at most one proposal per item
    n = len(claims)
    resolve.apply_links(store, cands, rep, [])  # deterministic ids: re-run adds nothing
    assert len(list(store.iter_claims(RelationType.FORMALIZED_AS))) == n


def test_curated_pairs_are_not_reproposed_and_evaluation(store: Store) -> None:
    from collections import Counter

    gold = binary("c:curated", RelationType.FORMALIZED_AS, "wd:Q11518",
                  "mathlib:EuclideanGeometry.dist_sq_eq_dist_sq_add_dist_sq_iff_angle_eq_pi_div_two", PROV)
    store.add_claim(gold)
    curated = resolve.curated_pairs(store)
    cands = list(resolve.formalization_candidates(store))
    ev = resolve.evaluate_against_curated(cands, curated)
    assert ev["gold_items"] == 1 and ev["recall_at_1"] == 1.0 and ev["precision_above_threshold"] == 1.0
    rep: Counter = Counter()
    resolve.apply_links(store, cands, rep, [], curated)
    assert rep["formalization_already_curated"] == 1


def test_area_candidates_link_mathlib_dirs_to_wikidata_areas(store: Store) -> None:
    cands = list(resolve.area_candidates(store))
    assert [(c.left, c.right, c.score) for c in cands] == [("mathlib-dir:Mathlib/NumberTheory", "wd:Q12479", 1.0)]


def test_run_end_to_end_reports(store: Store, tmp_path) -> None:
    out = resolve.run(store, queue_path=tmp_path / "queue.jsonl")
    assert out["counters"]["merges_applied"] == 1
    assert out["counters"]["formalization_claims_proposed"] >= 3
    assert out["counters"]["area_claims_proposed"] == 1
    assert (tmp_path / "queue.jsonl").exists()


def test_blocking_scales_to_many_declarations() -> None:
    """50k synthetic declarations x 5k items must resolve in seconds, not minutes (blocking works)."""
    rows = [resolve.DeclRow(f"mathlib:D{i}", f"Foo{i % 977}.bar_{i}_lemma", EntityType.STATEMENT,
                            resolve.decl_last_tokens(f"bar_{i}_lemma"), "", ()) for i in range(50_000)]
    idx = resolve.MathlibIndex(rows)
    t0 = time.perf_counter()
    n = sum(len(resolve.item_candidates(idx, f"wd:Q{i}", EntityType.CONCEPT, [(f"bar {i} lemma", 1.0)]))
            for i in range(5_000))
    elapsed = time.perf_counter() - t0
    assert n >= 5_000 and elapsed < 20.0


def test_whole_name_and_instances_break_last_component_ties() -> None:
    """Seen on the real atlas (2026-10-16): "uniform space" -> PiLp.uniformSpace instead of UniformSpace."""
    def row(name: str, kind: str) -> resolve.DeclRow:
        return resolve.DeclRow(f"mathlib:{name}", name, EntityType.DEFINITION, resolve.decl_last_tokens(name), "", (),
                               kind)

    idx = resolve.MathlibIndex([row("PiLp.uniformSpace", "instance"), row("UniformSpace", "class"),
                                row("Finpartition.toUniformSpace", "def")])
    (best, *rest) = resolve.item_candidates(idx, "wd:Q1", EntityType.CONCEPT, [("uniform space", 1.0)])
    assert best.right == "mathlib:UniformSpace" and best.score >= resolve.LINK_THRESHOLD
    assert all(c.score < resolve.LINK_THRESHOLD for c in rest)
    assert dict(next(c for c in rest if c.right == "mathlib:PiLp.uniformSpace").features)["instance"] is True
