"""r5: Spanish names from Wiktionary only when one translation table has a mathematical sense."""

import json

from atlas import Entity, EntityType, LangString, Provenance, Source, Store
from atlas.ingest import wiktionary as wk
from atlas.ingest.http import Response

T = "2026-10-16T00:00:00+00:00"
WD = Provenance("src:wikidata", "https://www.wikidata.org/wiki/Q1", "atlas.ingest.wikidata", "wd-map/2", "CC0-1.0", T)
ONE = """==English==
===Noun===
# {{lb|en|mathematics}} A thing.
====Translations====
{{trans-top|(mathematics) discrete subgroup of a vector space}}
* French: {{t+|fr|réseau|m}}
* Spanish: {{t+|es|retículo|m}}, {{t|es|red|f}}
{{trans-bottom}}
{{trans-top|structure of crossed strips}}
* Spanish: {{t+|es|enrejado|m}}
{{trans-bottom}}
==Spanish==
{{trans-top|(mathematics) wrong section}}
* Spanish: {{t|es|nope}}
{{trans-bottom}}
"""
TWO = ONE.replace("structure of crossed strips", "(order theory, mathematics) partially ordered set")


def test_only_one_math_table_yields_a_name():
    pick, why = wk.spanish_from(ONE)
    assert why == "ok" and pick.text == "retículo" and "discrete subgroup" in pick.gloss
    assert wk.spanish_from(TWO) == (None, "ambiguous_math_tables")
    assert wk.spanish_from("==English==\n{{trans-top|a bird}}\n* Spanish: {{t|es|pájaro}}\n{{trans-bottom}}")[1] \
        == "no_math_table"


class FakeClient:
    def get(self, url, params, **kw):
        pages = [{"title": "lattice", "revisions": [{"revid": 5, "slots": {"main": {"content": ONE}}}]}]
        return Response(json.dumps({"query": {"pages": pages}}).encode(), T, True)


def test_ingest_writes_a_reversible_presentation_label_not_the_items_label():
    st = Store()
    st.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    st.add_entity(Entity("wd:Q1", EntityType.CONCEPT, (LangString("lattice", "en"),), provenance=(WD,)))
    rep = wk.ingest(st, FakeClient())
    assert rep.counters["labels_written"] == 1
    (c,) = st.curations(target_id="wd:Q1")
    assert c.payload["text"] == "retículo" and c.payload["provenance"]["license"] == "CC-BY-SA-4.0"
    assert "oldid=5" in c.payload["provenance"]["locator"] and c.actor == wk.ACTOR
    assert st.get_entity("wd:Q1").label("es") == "lattice"  # the item's own labels are untouched
    assert wk.ingest(st, FakeClient()).counters["labels_looked_up"] == 0  # already labelled
    st.undo_curation(c.id, actor="AJ", reason="test")
    assert st.curations(target_id="wd:Q1") == []


def test_non_math_glosses_with_set_or_number_are_rejected():
    for gloss in ("set of pieces", "number of atoms in a series", "computing: number of pixels"):
        text = f"==English==\n{{{{trans-top|{gloss}}}}}\n* Spanish: {{{{t|es|x}}}}\n{{{{trans-bottom}}}}"
        assert wk.spanish_from(text)[1] == "no_math_table", gloss


def test_maxlag_body_is_rejected_not_counted_as_no_page():
    """MediaWiki answers maxlag with HTTP 200 + an error body: never cached, never 'no_page' (review MEDIUM)."""
    import pytest
    lag = json.dumps({"error": {"code": "maxlag", "info": "lagged"}}).encode()
    seen = {}

    class Lagged:
        def get(self, url, params, **kw):
            seen.update(kw)
            return Response(lag, T, False)
    with pytest.raises(RuntimeError, match="maxlag"):
        wk.fetch(Lagged(), ["lattice"])
    assert seen["accept"](lag) is False
