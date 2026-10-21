"""r5 point 3: fichas.  One `explains` claim per sentence, checked against the stored source text;
only a person publishes; «Qué saber antes» and «Con qué se conecta» cite existing claim ids."""

from pathlib import Path

import pytest

from atlas import Entity, EntityType, EvidenceState, LangString, Provenance, RelationType, Source, Store, ValidationError, binary
from atlas import fichas
from atlas.ingest import wikipedia as wp

T = "2026-10-16T00:00:00+00:00"
URL = wp.page_url("es", "Número primo", 175494931)
EXTRACT = ("En matemáticas, un número primo es un número natural mayor a 1 que tiene únicamente dos divisores "
           "positivos distintos: él mismo y el 1. El número 1, por convenio, no se considera ni primo ni compuesto.")
DRAFTS = Path(__file__).resolve().parent.parent / "docs" / "plans" / "r5" / "fichas"

MD = f"""---
concepto: wd:Q49008
titulo: número primo
version: ficha/1
autor: draft-llm/1
---
## Qué es
- Un número primo es un número natural mayor que 1 cuyos únicos divisores positivos son 1 y él mismo.
  fuente: src:wikipedia-es
  localizador: {URL} §1
  cita: un número primo es un número natural mayor a 1 que tiene únicamente dos divisores positivos distintos: él mismo y el 1.
- El 1 no es primo.
  fuente: src:wikipedia-es
  localizador: {URL} §1
  cita: «El número 1, por convenio, no se considera ni primo ni compuesto.»

## Cómo compruebo que lo entendí
- Pregunta: ¿es primo el 1? Respuesta: no, por convenio.
  fuente: src:wikipedia-es
  localizador: {URL} §1
  cita: El número 1, por convenio, no se considera ni primo ni compuesto.
"""


@pytest.fixture
def st():
    s = Store()
    s.add_source(Source("src:wikipedia-es", "Wikipedia (es)", "CC-BY-SA-4.0"))
    s.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    prov = Provenance("src:wikipedia-es", URL, wp.EXTRACTOR, wp.ADAPTER_VERSION, "CC-BY-SA-4.0", T)
    wd = Provenance("src:wikidata", "https://www.wikidata.org/wiki/Q49008", "atlas.ingest.wikidata", "wd-map/2", "CC0-1.0", T)
    frag = Entity(wp.fragment_id("es", "Número primo"), EntityType.SOURCE_FRAGMENT,
                  (LangString("Wikipedia (es): Número primo", "es"),), descriptions=(LangString(EXTRACT, "es"),),
                  provenance=(prov,))
    s.add_entities([frag, Entity("wd:Q49008", EntityType.CONCEPT, (LangString("número primo", "es"),)),
                    Entity("wd:Q50708", EntityType.CONCEPT, (LangString("divisor", "es"),)),
                    Entity("wd:Q21199", EntityType.CONCEPT, (LangString("número natural", "es"),))])
    s.add_claims([binary("c:wp", RelationType.APPEARS_IN, "wd:Q49008", frag.id, (prov,)),
                  binary("c:pre", RelationType.PREREQUISITE_FOR, "wd:Q50708", "wd:Q49008", (wd,)),
                  binary("c:sub", RelationType.SPECIAL_CASE_OF, "wd:Q49008", "wd:Q21199", (wd,))])
    return s


def test_import_writes_one_unreviewed_claim_per_sentence_with_its_source(st):
    counts = fichas.import_ficha(st, fichas.parse_markdown(MD), actor="atlas.fichas/1")
    assert counts == {"added": 3, "superseded": 0, "retracted": 0, "unchanged": 0}
    rows = fichas.current_sentences(st, "wd:Q49008")
    assert [n.note.section.value for _, n in rows] == ["que_es", "que_es", "compruebo"]
    for c, n in rows:
        assert c.evidence_state is EvidenceState.MODEL_PROPOSED
        p = c.provenance[0]
        assert (p.extractor, p.extractor_version, p.license, p.retrieved_at) == ("draft-llm", "1", "CC-BY-SA-4.0", T)
        assert p.quote and p.locator.startswith(URL)
        assert fichas.section_of(n.id) is n.note.section
    assert all(not h.entity_id.startswith("note:") for h in st.search("divisores positivos"))
    again = fichas.import_ficha(st, fichas.parse_markdown(MD), actor="atlas.fichas/1")
    assert again == {"added": 0, "superseded": 0, "retracted": 0, "unchanged": 3}


def test_public_view_shows_only_reviewed_sentences_and_cites_claim_ids(st):
    fichas.import_ficha(st, fichas.parse_markdown(MD), actor="atlas.fichas/1")
    public = fichas.render_ficha(st, "wd:Q49008")
    assert "únicos divisores positivos" not in public and fichas.DRAFT_LABEL not in public
    assert "estado: 0 de 3 frases revisadas" in public
    assert "c:pre" in public.split("## Qué saber antes")[1].split("##")[0]  # divisor, cited by id
    assert "c:sub" in public.split("## Con qué se conecta")[1] and "c:pre" not in public.split("## Con qué se conecta")[1]
    assert "Extracto de Wikipedia (es), sin revisar" in public and "CC-BY-SA-4.0" in public and "oldid=175494931" in public
    drafts = fichas.render_ficha(st, "wd:Q49008", include_drafts=True)
    assert fichas.DRAFT_LABEL in drafts and "únicos divisores positivos" in drafts and "model_proposed" in drafts
    first = fichas.current_sentences(st, "wd:Q49008")[0][0]
    with pytest.raises(ValidationError, match="person"):
        fichas.review(st, [first.id], reviewer="draft-llm", reason="x")
    fichas.review(st, [first.id], reviewer="adrian", reason="revisada contra la fuente")
    public = fichas.render_ficha(st, "wd:Q49008")
    assert "únicos divisores positivos" in public and "estado: 1 de 3" in public and "1 no es primo" not in public


def test_edit_supersedes_and_removal_retracts_keeping_history(st):
    fichas.import_ficha(st, fichas.parse_markdown(MD), actor="atlas.fichas/1")
    old = fichas.current_sentences(st, "wd:Q49008")[1][0]
    edited = MD.replace("- El 1 no es primo.", "- Por convenio, el 1 no es primo.")
    cut = edited.split("## Cómo compruebo")[0]
    counts = fichas.import_ficha(st, fichas.parse_markdown(cut), actor="atlas.fichas/1")
    assert counts == {"added": 0, "superseded": 1, "retracted": 1, "unchanged": 1}
    assert [c.evidence_state for c in st.history(old.id)] == [EvidenceState.SUPERSEDED, EvidenceState.MODEL_PROPOSED]
    assert len(fichas.current_sentences(st, "wd:Q49008")) == 2
    fichas.import_ficha(st, fichas.parse_markdown(MD), actor="atlas.fichas/1")  # the check comes back
    assert len(fichas.current_sentences(st, "wd:Q49008")) == 3


@pytest.mark.parametrize("change, error", [
    (("cita: El número 1, por convenio", "cita: El número 1, según Euclides"), "quote not found"),
    (("oldid=175494931 §1\n  cita: un", "oldid=1 §1\n  cita: un"), "stored extract is"),
    (("  cita: un número primo es un número natural mayor a 1 que tiene únicamente dos divisores positivos distintos: él mismo y el 1.\n", ""), "without quote"),
    (("autor: draft-llm/1", "autor: claude"), "autor"),
    (("- El 1 no es primo.", "- El 1 no es primo"), "end like a sentence"),
])
def test_unsourced_or_misquoted_sentences_are_rejected_before_writing(st, change, error):
    with pytest.raises(ValidationError, match=error):
        fichas.import_ficha(st, fichas.parse_markdown(MD.replace(*change)), actor="atlas.fichas/1")
    assert fichas.current_sentences(st, "wd:Q49008") == []


def test_pilot_drafts_parse_cover_the_five_questions_and_stay_drafts():
    files = sorted(DRAFTS.glob("*.md"))
    assert len(files) >= 20
    concepts = set()
    for f in files:
        d = fichas.parse_markdown(f.read_text(encoding="utf-8"))
        concepts.add(d.concept)
        assert d.author == "draft-llm/1", f  # written by a model: only Adrián's review publishes it
        assert {s.section for s in d.sentences} == set(fichas.FichaSection), f
    assert len(concepts) == len(files)


def test_conjunctive_claim_lists_every_premise(st):
    """'C depends on A and B jointly' must not render as 'C depends on A' (review MEDIUM, principle 3)."""
    from atlas import conjunction
    wd = Provenance("src:wikidata", "https://www.wikidata.org/wiki/Q49008", "atlas.ingest.wikidata", "wd-map/2", "CC0-1.0", T)
    st.add_claims([conjunction("c:and", RelationType.DEPENDS_ON_LOGICALLY, "wd:Q49008", ["wd:Q50708", "wd:Q21199"], (wd,))])
    before = "\n".join(fichas._claim_lines(st, "wd:Q49008", fichas.BEFORE, str))
    line = next(l for l in before.splitlines() if "c:and" in l)
    assert "divisor" in line and "número natural" in line and "2 premisas" in line
    inverse = "\n".join(fichas._claim_lines(st, "wd:Q50708", None, str))
    line = next(l for l in inverse.splitlines() if "c:and" in l)
    assert "número primo" in line and "número natural" in line
