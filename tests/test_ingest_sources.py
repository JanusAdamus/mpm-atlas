"""Wikidata + Wikipedia adapters and run_ingest end to end, offline.

Replays real responses recorded by tests/fixtures/ingest/record_fixtures.py
(Wikidata CC0, Wikipedia CC BY-SA 4.0) against the Mathlib fixture tree.
"""

from __future__ import annotations

import shutil
from pathlib import Path

import pytest

from atlas import EntityType, EvidenceState, RelationType, Role, Store
from atlas.ingest import run_ingest, wikidata, wikipedia
from atlas.ingest.http import CachedClient, OfflineMiss

from test_ingest_mathlib import COMMIT, build_tarball

FIXTURES = Path(__file__).parent / "fixtures" / "ingest"
E = EntityType
R = RelationType


def _fixture_seeds():
    import importlib.util

    spec = importlib.util.spec_from_file_location("record_fixtures", FIXTURES / "record_fixtures.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.FIXTURE_SEEDS


@pytest.fixture(scope="module")
def pipeline(tmp_path_factory: pytest.TempPathFactory):
    root = tmp_path_factory.mktemp("ingest")
    for name in ("wikidata", "wikipedia"):
        shutil.copytree(FIXTURES / "cache" / name, root / name)
    build_tarball(root)
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(wikidata, "SEEDS", _fixture_seeds())
        store = Store()
        out = run_ingest(store, offline=True, cache_root=root, workers=1)
        yield store, out, root
        store.close()


def _report(out: dict, source: str) -> dict:
    return next(r for r in out["reports"] if r["source"] == source)["counters"]


# ---------------------------------------------------------------------- mapping table (pure)


@pytest.mark.parametrize("pid, subj, obj, expected", [
    ("P279", E.AREA, E.AREA, R.PART_OF_AREA),
    ("P279", E.CONCEPT, E.CONCEPT, R.SPECIAL_CASE_OF),
    ("P31", E.CONCEPT, E.AREA, R.PART_OF_AREA),
    ("P138", E.STATEMENT, E.PERSON, R.NAMED_AFTER),
    ("P1318", E.STATEMENT, E.PERSON, R.AUTHORED_BY),
    ("P7719", E.STATEMENT, E.STATEMENT, R.GENERALIZES),
    ("P366", E.CONCEPT, E.APPLICATION, R.APPLIED_IN),
    ("P2283", E.STATEMENT, E.CONCEPT, R.PREREQUISITE_FOR),
])
def test_property_map(pid, subj, obj, expected) -> None:
    rule, why = wikidata.map_statement(pid, subj, obj)
    assert why == "mapped" and rule.relation is expected


def test_property_map_drops_what_does_not_fit() -> None:
    assert wikidata.map_statement("P138", E.STATEMENT, E.CONCEPT) == (None, "endpoint_types")  # not a person
    assert wikidata.map_statement("P366", E.CONCEPT, E.CONCEPT) == (None, "endpoint_types")  # use must be WORLD
    assert wikidata.map_statement("P9999", E.CONCEPT, E.CONCEPT) == (None, "unmapped")
    assert wikidata.map_statement("P2283", E.STATEMENT, E.CONCEPT)[0].inverse  # used item is the prerequisite
    assert {r["version"] for r in wikidata.mapping_table()} == {wikidata.MAPPING_VERSION}


def _item(qid: str, statements=(), literals=(), descr: str | None = None) -> wikidata.ItemData:
    return wikidata.ItemData(qid, {"en": qid}, [], {"en": descr} if descr else {}, {}, list(literals),
                             list(statements), "2026-10-16T00:00:00+00:00")


def test_broad_seed_items_need_their_own_math_signal() -> None:
    from atlas.ingest.common import IngestReport

    items = {
        "Q1": _item("Q1", descr="day of the year"),  # precise seed: kept whatever it says
        "Q2": _item("Q2", literals=[("P2812", "Circle")]),  # broad, MathWorld id
        "Q3": _item("Q3", descr="teorema de geometría plana"),  # broad, Spanish math description
        "Q4": _item("Q4", statements=[("P279", "Q2")], descr="pin that holds connecting rod to piston head"),
        "Q5": _item("Q5", literals=[("P6781", "Mathematician:Mathematicians/Sorted_By_Nation/Egypt")]),
    }
    seed_of = {"Q1": "theorem", "Q2": "obj", "Q3": "obj", "Q4": "obj", "Q5": "obj"}
    rep = IngestReport("t")
    kept = wikidata.in_scope(items, seed_of, frozenset({"obj"}), rep)
    assert set(kept) == {"Q1", "Q2", "Q3"}  # a P279 hop to a math item is not enough on its own
    assert rep.counters["seed:obj:out_of_scope"] == 2
    assert rep.counters["scope_signal:id:P2812"] == 1 and rep.counters["scope_signal:description"] == 1


def test_scope_type_drops_wikimedia_pages_and_types_humans() -> None:
    assert wikidata.scope_type(_item("Q9", [("P31", "Q4167836")]), E.MODEL) is None  # Wikimedia category
    assert wikidata.scope_type(_item("Q9", [("P31", "Q5")]), E.CONCEPT) is E.PERSON
    assert wikidata.scope_type(_item("Q9", [("P31", "Q11518")]), E.CONCEPT) is E.CONCEPT


# ---------------------------------------------------------------------- Wikidata


def test_wikidata_entities_are_bilingual_with_ids(pipeline) -> None:
    store, out, _ = pipeline
    pyth = store.get_entity("wd:Q11518")
    assert pyth.type is E.STATEMENT
    assert {l.lang for l in pyth.labels} == {"en", "es"} and pyth.label("en") == "Pythagorean theorem"
    ext = dict(pyth.external_ids)
    assert ext["wikidata"] == "Q11518" and "enwiki" in ext and "eswiki" in ext
    assert pyth.provenance[0].license == "CC0-1.0"
    assert store.get_entity("wd:Q12479").type is E.AREA
    assert store.get_source("src:wikidata").license == "CC0-1.0"


def test_wikidata_claims_are_mapped_and_unmapped_are_counted(pipeline) -> None:
    store, out, _ = pipeline
    counters = _report(out, "wikidata")
    assert any(k.startswith("unmapped:") for k in counters)
    claims = [c for c in store.iter_claims() if c.provenance[0].source_id == "src:wikidata"]
    assert claims
    for c in claims:
        p = c.provenance[0]
        assert p.extractor_version == wikidata.MAPPING_VERSION and p.locator.startswith("https://www.wikidata.org/wiki/Q")
    named = store.claims_for("wd:Q11518", roles=[Role.SUBJECT], relations=[R.NAMED_AFTER])
    assert named and store.get_entity(named[0].targets[0]).type is E.PERSON  # fetched by the closure step


def test_curated_mathlib_links_are_written_after_wikidata(pipeline) -> None:
    store, out, _ = pipeline
    links = {(c.subject, c.targets[0]): c for c in store.iter_claims(R.FORMALIZED_AS)
             if c.provenance[0].source_id == "src:mathlib4"}
    c = links[("wd:Q49008", "mathlib:Nat.Prime")]  # @[wikidata Q49008]
    assert c.evidence_state is EvidenceState.EXTRACTED and c.formal.version == COMMIT
    assert ("wd:Q11518", "mathlib:EuclideanGeometry.dist_sq_eq_dist_sq_add_dist_sq_iff_angle_eq_pi_div_two") in links


def test_offline_miss_is_explicit(tmp_path: Path) -> None:
    client = CachedClient(tmp_path / "empty", offline=True)
    with pytest.raises(OfflineMiss):
        client.get("https://example.org/x")
    # an uncached seed query replays the online run's timeout fallback: skipped, counted and noted
    rep = wikidata.ingest(Store(), client=client, seeds=(wikidata.Seed("x", E.CONCEPT, "VALUES ?item { wd:Q1 }"),))
    assert rep.counters["seed:x:failed_timeout"] == 1
    assert any("seed x skipped" in f for f in rep.failures)
    # a single-item detail query has no fallback: the miss still surfaces
    with pytest.raises(OfflineMiss):
        wikidata.fetch_details(wikidata.Sparql(client), ["Q1"], rep)


# ---------------------------------------------------------------------- Wikipedia


def test_wikipedia_fragments_carry_attribution(pipeline) -> None:
    store, out, _ = pipeline
    frags = list(store.iter_entities(E.SOURCE_FRAGMENT))
    assert frags and {f.labels[0].lang for f in frags} == {"en", "es"}
    for f in frags:
        p = f.provenance[0]
        assert p.license == "CC-BY-SA-4.0" and "oldid=" in p.locator
    assert any(f.descriptions for f in frags)  # lead extracts
    (app,) = [c for c in store.claims_for("wd:Q11518", roles=[Role.SUBJECT], relations=[R.APPEARS_IN])
              if c.provenance[0].source_id == "src:wikipedia-en"]
    assert store.get_entity(app.targets[0]).type is E.SOURCE_FRAGMENT


def test_wikipedia_links_are_only_model_proposed_citations(pipeline) -> None:
    store, out, _ = pipeline
    cites = list(store.iter_claims(R.CITES))
    assert cites
    assert all(c.evidence_state is EvidenceState.MODEL_PROPOSED for c in cites)
    assert all(c.explanation and "never" in c.explanation[0].text.lower() + c.explanation[1].text.lower()
               for c in cites)
    assert not [c for c in store.iter_claims(R.PREREQUISITE_FOR) if c.provenance[0].source_id.startswith("src:wiki")
                and c.provenance[0].source_id != "src:wikidata"]


# ---------------------------------------------------------------------- end to end


def test_run_ingest_resolves_and_reports(pipeline) -> None:
    store, out, _ = pipeline
    assert out["resolve"]["counters"]["merges_applied"] == 1  # the Mathlib alias
    assert store.resolve("mathlib:Nat.prime_two_le") == "mathlib:Nat.Prime.two_le"
    cov = out["coverage"]
    assert {"src:mathlib4", "src:wikidata", "src:wikipedia-en", "src:wikipedia-es"} <= set(cov["claims_by_source"])
    assert out["elapsed_s"] >= 0


def test_run_ingest_is_idempotent(pipeline) -> None:
    store, out, root = pipeline
    with pytest.MonkeyPatch.context() as mp:
        mp.setattr(wikidata, "SEEDS", _fixture_seeds())
        again = run_ingest(store, offline=True, cache_root=root, workers=1)
    assert again["coverage"] == out["coverage"]


def test_unknown_source_is_rejected() -> None:
    with pytest.raises(ValueError):
        run_ingest(Store(), ["nope"])


def test_wikipedia_targets_come_from_sitelinks(pipeline) -> None:
    store, _, _ = pipeline
    t = wikipedia.targets_from_store(store)
    assert t["en"].get("Pythagorean theorem") == "wd:Q11518"
    assert all(not v.startswith("wd:Q10261") for v in t["en"].values())  # people are skipped


# ---------------------------------------------------------------------- Wikipedia politeness and priority


def test_wikipedia_politeness_follows_contact() -> None:
    fast, workers = wikipedia.polite_settings(identified=True)
    slow, one = wikipedia.polite_settings(identified=False)
    assert workers <= 3 and 60 / fast < 200  # the interval is shared by all workers: aggregate under 200/min
    assert one == 1 and 60 / slow < 10  # under the 10/min bucket of a UA without contact


def test_wikipedia_titles_are_prioritized_and_batches_are_stable() -> None:
    from collections import Counter

    titles = {f"T{i:03d}": f"wd:Q{i}" for i in range(120)}
    degree = Counter({"wd:Q7": 50, "wd:Q99": 40})
    order = wikipedia.prioritized(titles, degree)
    assert order[:3] == ["T007", "T099", "T000"]
    plan = wikipedia.plan_batches(order, done={"T007", "T001"})
    assert [len(b) for b in plan] == [50, 50, 18, 2]
    assert plan[-1] == ["T001", "T007"]  # titles already in the store go last
    assert all(b == sorted(b) for b in plan)  # sorted inside a batch: same URL, same cache key
    assert "T099" in plan[0] and sum(len(b) for b in plan) == 120


def test_wikipedia_keeps_only_article_links() -> None:
    keep = [l for l in ["Wikipedia:Good_articles*", "Category:Theorems", "Categoría:Teoremas", "Euclid's_Elements",
                        "Ratio: a test", "Special:BookSources"] if wikipedia._is_article(l)]
    assert [wikipedia._article_title(l) for l in keep] == ["Euclid's Elements", "Ratio: a test"]


def test_wikipedia_resume_links_new_pages_to_earlier_fragments(pipeline) -> None:
    """A page fetched in a later run links to fragments written by an earlier run (and through redirects)."""
    store, _, root = pipeline
    items, frags = wikipedia.existing(store, "en")
    assert "wd:Q11518" in items and "Pythagorean theorem" in frags
    cites = [c for c in store.iter_claims(R.CITES) if c.provenance[0].source_id == "src:wikipedia-en"]
    linked = {c.targets[0] for c in cites}
    assert frags["Pythagorean theorem"] in linked or any(c.subject == frags["Pythagorean theorem"] for c in cites)
    client = CachedClient(root / "wikipedia", offline=True)
    rep = wikipedia.ingest(store, client=client, langs=("en",))
    assert rep.counters["en:titles_already_in_store"] == rep.counters["en:titles_requested"]
    assert rep.counters["en:fragments"] == 0 and rep.counters["claims_written"] == 0  # idempotent re-run


def test_wikipedia_offline_replay_skips_batches_never_fetched(tmp_path: Path) -> None:
    client = CachedClient(tmp_path / "empty", offline=True)
    assert wikipedia._fetch_or_skip(client, "en", ["Never fetched"]) is None


def test_wikipedia_zero_budget_writes_nothing_and_counts_skips(pipeline) -> None:
    store, _, root = pipeline
    client = CachedClient(root / "wikipedia", offline=True)
    rep = wikipedia.ingest(store, client=client, budget_s=0.0)
    assert rep.counters["en:titles_skipped_budget"] == rep.counters["en:titles_requested"] > 0
    assert rep.counters["claims_written"] == 0


def test_wikipedia_links_pages_of_earlier_runs_to_new_pages(tmp_path: Path) -> None:
    """Run 1 fetched "Beta" (which links to "Alpha"); run 2 fetches only "Alpha".  The Beta -> Alpha
    candidate comes from Beta's cached response, without fetching Beta again."""
    import json as _json

    from atlas import Entity, LangString
    from atlas.ingest.http import Response, _write_cached

    def body(title: str, links: list[str]) -> bytes:
        page = {"title": title, "ns": 0, "lastrevid": 7, "cirrusdoc": [{"source": {
            "opening_text": f"{title} is a test.", "outgoing_link": [l.replace(" ", "_") for l in links]}}]}
        return _json.dumps({"query": {"pages": [page]}}).encode()

    class Client:  # serves canned bodies and keeps them in an HTTP-cache layout, as CachedClient does
        cache_dir = tmp_path / "wikipedia"
        stats: dict = {}

        def get(self, url, params, accept=None):
            title = params["titles"]
            data = body(title, ["Alpha"] if title == "Beta" else [])
            _write_cached(self.cache_dir / "00" / f"{title}.gz", data, "2026-10-17T00:00:00+00:00", "GET",
                          f"{url}?titles={title}", None)
            return Response(data, "2026-10-17T00:00:00+00:00", False)

    def item(qid: str, title: str) -> Entity:
        return Entity(f"wd:{qid}", E.CONCEPT, (LangString(title, "en"),), external_ids=(("enwiki", title),))

    store = Store()
    store.add_entity(item("Q2", "Beta"))
    first = wikipedia.ingest(store, client=Client(), langs=("en",))
    assert first.counters["en:fragments"] == 1 and first.counters["en:cites_candidates"] == 0
    store.add_entity(item("Q1", "Alpha"))
    second = wikipedia.ingest(store, client=Client(), langs=("en",), limit=1)
    assert second.counters["en:titles_requested"] == 1 and second.counters["en:pages_from_earlier_runs"] == 1
    (cite,) = list(store.iter_claims(R.CITES))
    assert (cite.subject, cite.targets[0]) == (wikipedia.fragment_id("en", "Beta"), wikipedia.fragment_id("en", "Alpha"))
    assert cite.evidence_state is EvidenceState.MODEL_PROPOSED
