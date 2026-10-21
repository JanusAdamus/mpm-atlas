"""Works adapters (ADR-0005 §8): metadata-only filters, dates, idempotent batches, DOI merges.  Offline."""

from __future__ import annotations

from pathlib import Path

import pytest

from atlas.ingest.http import HttpError
from atlas.ingest.works import canon, common, coverage, openalex, resolve, zbmath
from atlas.model import Entity, EntityType, LangString, ValidationError
from atlas.store import Store

ZB_RECORD = {
    "id": 7224945, "identifier": "1234.30001", "year": "2020",
    "title": {"title": "On entire functions", "subtitle": None, "addition": None},
    "document_type": {"code": "j"}, "language": {"languages": ["English"]},
    "links": [{"type": "doi", "identifier": "https://doi.org/10.1/ABC"}, {"type": "url", "identifier": "x"}],
    "contributors": {"authors": [{"codes": ["doe.jane"], "name": "Doe, Jane"}], "editors": []},
    "msc": [{"code": "30D20"}], "source": {"source": "J. Anal."}, "database": "Zbl",
    "references": [{"zbmath": {"document_id": 1}, "doi": None}],
    "editorial_contributions": [{"text": "A review with its own rights"}], "keywords": ["entire"],
}


def test_zbmath_filter_keeps_metadata_only() -> None:
    r = zbmath._filter(ZB_RECORD)
    assert "editorial_contributions" not in r and "keywords" not in r
    assert r["doi"] == ["10.1/abc"] and r["authors"] == [["doe.jane", "Doe, Jane"]] and r["msc"] == ["30D20"]


def test_metadata_only_guard_rejects_text() -> None:
    e = Entity("zb:1", EntityType.PUBLICATION, (LangString("t", "en"),), descriptions=(LangString("abstract", "en"),))
    with pytest.raises(ValidationError):
        common.assert_metadata_only([e])


def test_cdli_date_is_astronomical_and_approximate() -> None:
    assert canon.cdli_date("ca. 1900-1600 BC") == "-1899~/-1599~"
    assert canon.cdli_date(None) is None


def test_openalex_language_and_early_year_flagged() -> None:
    assert openalex._lang("FR") == "fr" and openalex._lang("zz-x") == "und" and openalex._lang(None) == "und"
    p = common.prov("openalex", "https://openalex.org/W1", "x", "1", "2026-10-20T00:00:00+00:00")
    dates, flagged = openalex._date({"year": 1200, "date": None}, p)
    assert flagged and dates[0].edtf == "1200?" and dates[0].calendar == "unknown"


def test_openalex_429_count_is_unknown(monkeypatch: pytest.MonkeyPatch) -> None:
    class Refusing:
        def get(self, *a: object, **k: object) -> object:
            raise HttpError(429, "https://api.openalex.org/works", b"budget")
    assert coverage._oa_count(Refusing(), 1900, 1949) is None  # type: ignore[arg-type]


def _store(tmp_path: Path) -> Store:
    st = Store(tmp_path / "w.db")
    common.register_sources(st, ["zbmath", "openalex"])
    return st


def test_batch_is_idempotent_and_doi_merge_prefers_zbmath(tmp_path: Path) -> None:
    st = _store(tmp_path)
    cached = common.CachedBatch(tmp_path / "x", [], "0" * 64)
    doc, people = zbmath._entities({**zbmath._filter(ZB_RECORD), "retrieved_at": "2026-10-20T00:00:00+00:00"})
    c = common.claim(zbmath.R.AUTHORED_BY, doc.id, people[0].id, doc.provenance[0])
    assert common.run_batch(st, "zbmath", "r", "k", cached, [doc, *people], [c], {}) is not None
    assert common.run_batch(st, "zbmath", "r", "k", cached, [doc, *people], [c], {}) is None  # skipped, no rows
    p = common.prov("openalex", "https://openalex.org/W9", "x", "1", "2026-10-20T00:00:00+00:00")
    oa = Entity("oa:W9", EntityType.PUBLICATION, (LangString("t", "en"),), external_ids=(("doi", "10.1/abc"),),
                provenance=(p,))
    common.run_batch(st, "openalex", "r", "k", cached, [oa], [], {})
    assert resolve.doi_merges(st) == [("zb:7224945", "oa:W9", "doi:10.1/abc")]
    st.close()


def test_zbmath_short_partition_is_not_cached(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """An empty page before `nr_total_results` is reached must fail, not freeze a short cache (review HIGH)."""
    import json as _json
    from atlas.ingest.http import Response
    monkeypatch.setattr(common, "WORKS_CACHE", tmp_path)
    pages = [[ZB_RECORD, {**ZB_RECORD, "id": 2}], []]

    class Short:
        def get(self, url: str, params: dict[str, str], **k: object) -> Response:
            body = {"status": {"execution_bool": True, "nr_total_results": 3}, "result": pages[int(params["page"])]}
            return Response(_json.dumps(body).encode(), "2026-10-20T00:00:00+00:00", False)
    with pytest.raises(RuntimeError, match="3"):
        zbmath.fetch_partition(Short(), "30", 2020)  # type: ignore[arg-type]
    assert not common.cache_path("zbmath", zbmath.partition_key("30", 2020)).exists()


def test_resolve_lists_are_computed_after_the_previous_merge(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """An edition merged into its work must not reappear as a DOI survivor in the same run (review MEDIUM)."""
    st = _store(tmp_path)
    cached = common.CachedBatch(tmp_path / "x", [], "0" * 64)
    p = common.prov("openalex", "https://openalex.org/W9", "x", "1", "2026-10-20T00:00:00+00:00")
    pub = lambda i, *ids: Entity(i, EntityType.PUBLICATION, (LangString(i, "en"),), external_ids=ids, provenance=(p,))
    common.run_batch(st, "openalex", "r", "k", cached,
                     [pub("wdw:W"), pub("wdw:E", ("doi", "10.1/x")), pub("oa:X", ("doi", "10.1/x"))], [], {})
    monkeypatch.setattr(canon, "load_curated", lambda: {})
    monkeypatch.setattr(canon, "edition_merges", lambda store, cur: [("wdw:W", "wdw:E", "edition_of")])
    monkeypatch.setattr(canon, "cdli_merges", lambda store: [])
    monkeypatch.setattr(canon, "person_merges", lambda store: [])
    out = resolve.run(st)
    assert out["edition_of"] == 1 and out["doi"] == 0
    st.close()


def test_rerun_with_changed_input_is_not_skipped_silently(tmp_path: Path) -> None:
    """Same (source, run, key) with another input_sha256 must fail loudly, not count as skipped (review MEDIUM)."""
    st = _store(tmp_path)
    common.run_batch(st, "zbmath", "r", "k", common.CachedBatch(tmp_path / "x", [], "0" * 64), [], [], {})
    assert common.run_batch(st, "zbmath", "r", "k", common.CachedBatch(tmp_path / "x", [], "0" * 64), [], [], {}) is None
    with pytest.raises(ValidationError, match="changed"):
        common.run_batch(st, "zbmath", "r", "k", common.CachedBatch(tmp_path / "x", [], "1" * 64), [], [], {})
    st.close()


def test_citation_fetch_with_http_errors_is_not_cached(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A 429/5xx must not freeze an empty reference list in the cache (review MEDIUM)."""
    from atlas.ingest.works import citations
    monkeypatch.setattr(common, "WORKS_CACHE", tmp_path)
    monkeypatch.setattr(citations, "WORKS_CACHE", tmp_path)

    class Busy:
        def __init__(self, *a: object, **k: object) -> None: ...
        def get(self, url: str, *a: object, **k: object) -> object:
            raise HttpError(429, url, b"slow down")
    monkeypatch.setattr(citations, "CachedClient", Busy)
    with pytest.raises(RuntimeError, match="429|HTTP"):
        citations.fetch("opencitations", "s", ["10.1/a"])
    assert not common.cache_path("opencitations", "s").exists()


def test_openalex_doi_with_a_zbmath_record_is_secondary(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """An existing oa: record for the DOI must not hide the zb: primary record (review MEDIUM, ADR-0005 §8.1)."""
    st = _store(tmp_path)
    monkeypatch.setattr(common, "WORKS_CACHE", tmp_path)
    monkeypatch.setattr(openalex, "WORKS_CACHE", tmp_path)
    cached = common.CachedBatch(tmp_path / "x", [], "0" * 64)
    for src, i in (("openalex", "oa:W1"), ("zbmath", "zb:1")):
        p = common.prov(src, "https://example.org/" + i, "x", "1", "2026-10-20T00:00:00+00:00")
        common.run_batch(st, src, "seed", i, cached, [Entity(i, EntityType.PUBLICATION, (LangString(i, "en"),),
                                                             external_ids=(("doi", "10.1/x"),), provenance=(p,))], [], {})
    row = {"id": "W2", "doi": "10.1/x", "title": "t", "language": "en", "subfield": None, "date": None,
           "year": 2020, "authors": [["A1", "Doe", None]], "refs": ["W1"], "retrieved_at": "2026-10-20T00:00:00+00:00"}
    common.write_cache(common.cache_path("openalex", "b"), [row])
    out = openalex.ingest_works(st, "b")
    assert out["secondary"] == 1 and out["claims"] == 0
    cit = openalex.ingest_citations(st, "b")
    assert cit["from_secondary"] == 1 and cit["claims"] == 0
    st.close()


def test_zbmath_count_404_without_no_results_is_unknown() -> None:
    """Only the 'No results found' envelope is a known zero (review MEDIUM)."""
    class NotFound:
        def __init__(self, body: bytes) -> None:
            self.body = body
        def get(self, url: str, *a: object, **k: object) -> object:
            raise HttpError(404, url, self.body)
    assert coverage._zb_count(NotFound(b'{"status":{"internal_code":"No results found"}}'), "q") == 0  # type: ignore[arg-type]
    assert coverage._zb_count(NotFound(b"<html>moved</html>"), "q") is None  # type: ignore[arg-type]


def test_cdli_indexed_is_read_from_the_cache(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """The CDLI count comes from the fetched pages and their date, not a literal (review MEDIUM/LOW)."""
    monkeypatch.setattr(common, "WORKS_CACHE", tmp_path)
    monkeypatch.setattr(coverage, "WORKS_CACHE", tmp_path)
    assert coverage._cdli_indexed()[0] is None
    for page, n in ((1, 2), (2, 1)):
        common.write_cache(common.cache_path("cdli", f"genre=Mathematical;page={page:03d}"),
                           [{"id": i, "retrieved_at": f"2026-10-1{page}T00:00:00+00:00"} for i in range(n)])
    n, method = coverage._cdli_indexed()
    assert n == 3 and "2 pages" in method and "2026-10-12" in method
