"""r5: Spanish labels from Wikidata (labels, variants, lexemes, aliases), keyed by QID, with history."""

import json

from atlas import Entity, EntityType, EventKind, LangString, Provenance, Source, Store
from atlas.ingest import labels_es as les
from atlas.ingest.http import Response

T = "2026-10-16T00:00:00+00:00"
WD = Provenance("src:wikidata", "https://www.wikidata.org/wiki/Q1", "atlas.ingest.wikidata", "wd-map/2", "CC0-1.0", T)


def _row(qid, kind, v, lang="", ref=None):
    r = {"item": {"value": f"http://www.wikidata.org/entity/{qid}"}, "kind": {"value": kind},
         "v": {"value": v, **({"xml:lang": lang} if lang else {})}}
    if ref:
        r["ref"] = {"value": f"http://www.wikidata.org/entity/{ref}"}
    return r


class FakeClient:
    def __init__(self, rows):
        self.rows, self.calls = rows, 0

    def post(self, url, data, **kw):
        self.calls += 1
        return Response(json.dumps({"results": {"bindings": self.rows}}).encode(), T, True)


def test_choose_prefers_label_then_lexeme_and_skips_ambiguous_lexemes():
    rows = [_row("Q1", "lexeme", "retículo", ref="L1"), _row("Q1", "alias", "red cristalina", "es"),
            _row("Q2", "lexeme", "haz", ref="L2"), _row("Q2", "lexeme", "gavilla", ref="L3"),
            _row("Q2", "alias", "haz de grupos", "es"), _row("Q3", "label", "Trie", "mul"),
            _row("Q4", "label", "anillo", "es"), _row("Q4", "lexeme", "anillo", ref="L4")]
    c = les.candidates(rows)
    assert les.choose(c["Q1"])[:1] == ("retículo",) and les.choose(c["Q1"])[2] == "lexeme"
    assert les.choose(c["Q2"])[2] == "alias"  # two different lexemes: ambiguous, the single alias wins
    assert les.choose(c["Q3"])[2] == "mul" and les.choose(c["Q4"])[2] == "label"
    assert c["Q1"]["lexeme"][0][1] == "https://www.wikidata.org/wiki/Lexeme:L1"


def test_ingest_writes_es_label_with_provenance_and_previous():
    st = Store()
    st.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    st.add_entities([Entity("wd:Q1", EntityType.CONCEPT, (LangString("lattice", "en"),), provenance=(WD,)),
                     Entity("wd:Q9", EntityType.CONCEPT, (LangString("thing", "en"),), provenance=(WD,))])
    assert les.missing_es(st) == ["wd:Q1", "wd:Q9"]
    client = FakeClient([_row("Q1", "lexeme", "retículo", ref="L1"), _row("Q1", "alias", "red", "es")])
    rep = les.ingest(st, client)
    e = st.get_entity("wd:Q1")
    assert e.label("es") == "retículo" and ("es", "red") in {(a.lang, a.text) for a in e.aliases}
    p = e.provenance[-1]
    assert p.extractor == les.EXTRACTOR and p.license == "CC0-1.0" and p.locator.endswith("Lexeme:L1")
    (ev,) = st.events("wd:Q1", kinds=[EventKind.ENTITY_REPLACED])
    assert "es" not in ev.payload["previous"]["labels"]
    assert rep.counters["labels_written"] == 1 and les.missing_es(st) == ["wd:Q9"]
    assert les.ingest(st, client).counters["labels_written"] == 0  # re-run: nothing new


def test_mul_label_with_plain_english_words_is_not_a_spanish_name():
    assert les.mul_is_name("Fortran") and les.mul_is_name("gpt-oss") and les.mul_is_name("𝐿(ℝ)")
    assert not les.mul_is_name("Alternating polynomial") and not les.mul_is_name("New Zealand half crown")
    c = les.candidates([_row("Q5", "label", "Munching square", "mul")])
    assert les.choose(c["Q5"]) is None
