"""r5: Wikipedia extracts end at a sentence boundary, keep their licence, and the old mid-sentence cuts are
repaired from the cached text of the cited revision (offline)."""

import json

from atlas import Entity, EntityType, EventKind, LangString, Provenance, RelationType, Source, Store, binary
from atlas.ingest import wikipedia as wp
from atlas.ingest.http import CachedClient, _write_cached

T = "2026-10-16T00:00:00+00:00"
SENT = "La integral es un concepto fundamental del cálculo. "


def _long(n: int = 40) -> str:
    return "".join(f"{SENT[:-2]} número {i}. " for i in range(n)).strip()


def test_cut_ends_at_a_sentence_and_never_mid_word():
    text = _long()
    assert len(text) > wp.MAX_EXTRACT
    cut = wp.cut_at_sentence(text)
    assert len(cut) <= wp.MAX_EXTRACT and cut.endswith(".") and text.startswith(cut)
    assert text[: wp.MAX_EXTRACT] != cut  # the wikipedia/2 cut was mid-sentence


def test_cut_skips_abbreviations_and_initials():
    text = "Fue probado por J. Smith en 1900. Xxxxxxxx, p. ej. Algo y más " + "a " * 900
    cut = wp.cut_at_sentence(text, limit=120)
    assert cut == "Fue probado por J. Smith en 1900."


def test_cut_without_boundary_marks_the_ellipsis():
    cut = wp.cut_at_sentence("palabra " * 400, limit=100)
    assert cut.endswith(" …") and len(cut) <= 100


def test_short_text_is_untouched():
    assert wp.cut_at_sentence("  Una  frase.  ") == "Una frase."


def test_repair_recuts_from_the_same_revision_and_keeps_history(tmp_path):
    full = _long()
    client = CachedClient(tmp_path, offline=True)
    url = wp.api_url("es") + "?titles=Integral"
    body = {"query": {"pages": [{"title": "Integral", "lastrevid": 42,
                                  "cirrusdoc": [{"source": {"opening_text": full}}]},
                                 {"title": "Integral", "lastrevid": 41,
                                  "cirrusdoc": [{"source": {"opening_text": "Otra revisión. " * 200}}]}]}}
    _write_cached(client.cache_path("k1"), json.dumps(body).encode(), T, "GET", url, None)
    st = Store()
    st.add_source(Source("src:wikipedia-es", "Wikipedia (es)", "CC-BY-SA-4.0"))
    st.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    prov = Provenance("src:wikipedia-es", wp.page_url("es", "Integral", 42), wp.EXTRACTOR, "wikipedia/2",
                      "CC-BY-SA-4.0", T)
    frag = Entity(wp.fragment_id("es", "Integral"), EntityType.SOURCE_FRAGMENT,
                  (LangString("Wikipedia (es): Integral", "es"),),
                  descriptions=(LangString(full[: wp.MAX_EXTRACT], "es"),), provenance=(prov,))
    item = Entity("wd:Q80091", EntityType.CONCEPT, (LangString("integración", "es"),))
    st.add_entities([frag, item])
    st.add_claim(binary("c:a", RelationType.APPEARS_IN, item.id, frag.id, (prov,)))
    rep = wp.repair_extracts(st, client, langs=("es",))
    assert rep.counters["es:replaced"] == 1 and rep.counters["es:sentence_cut"] == 1
    got = st.get_entity(frag.id)
    assert got.descriptions[0].text == wp.cut_at_sentence(full)
    assert got.provenance[0].license == "CC-BY-SA-4.0" and got.provenance[0].locator.endswith("oldid=42")
    assert got.provenance[0].extractor_version == wp.ADAPTER_VERSION
    (ev,) = st.events(frag.id, kinds=[EventKind.ENTITY_REPLACED])
    assert ev.payload["previous"]["descriptions"]["es"] == full[: wp.MAX_EXTRACT]
    assert wp.repair_extracts(st, client, langs=("es",)).counters["es:replaced"] == 0  # idempotent
    lead = wp.lead_extract(st, item.id, "es")
    assert lead["complete"] and lead["license"] == "CC-BY-SA-4.0" and "oldid=42" in lead["attribution"]


def test_repair_without_cached_revision_cuts_inside_the_stored_prefix(tmp_path):
    full = _long()
    st = Store()
    st.add_source(Source("src:wikipedia-en", "Wikipedia (en)", "CC-BY-SA-4.0"))
    prov = Provenance("src:wikipedia-en", wp.page_url("en", "X", 7), wp.EXTRACTOR, "wikipedia/2", "CC-BY-SA-4.0", T)
    frag = Entity(wp.fragment_id("en", "X"), EntityType.SOURCE_FRAGMENT, (LangString("Wikipedia (en): X", "en"),),
                  descriptions=(LangString(full[: wp.MAX_EXTRACT], "en"),), provenance=(prov,))
    st.add_entity(frag)
    rep = wp.repair_extracts(st, CachedClient(tmp_path, offline=True), langs=("en",))
    assert rep.counters["en:no_cached_revision"] == 1 and rep.counters["en:replaced"] == 1
    text = st.get_entity(frag.id).descriptions[0].text
    assert text.endswith(".") and full.startswith(text)
