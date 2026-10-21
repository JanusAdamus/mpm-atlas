"""r5: Mathlib topic lists (three levels) link usual names to declarations as aliases, with history."""

from atlas import Entity, EntityType, EventKind, LangString, Provenance, Source, Store
from atlas.ingest import mathlib_topics as mt

YAML = """# comment
Linear algebra:
  Fundamentals:
    vector space: 'Module'
    "$p$-adic number": 'Padic'
    quotient: ''
    rank of a system: 'https://example.org/x.pdf'
  Duality:
    dual vector space: 'Module.Dual'   # trailing comment
Topology:
  Streams:
    stream: 'Stream'''
    classification of abelian simple groups: CommGroup.is_simple_iff_prime_card
    two decls:
      'A.one'
      'A.two'
"""


def test_parse_reads_three_levels_quotes_comments_and_continuations():
    t = {(x.path, x.decl) for x in mt.parse(YAML)}
    assert (("Linear algebra", "Fundamentals", "vector space"), "Module") in t
    assert (("Linear algebra", "Fundamentals", "$p$-adic number"), "Padic") in t
    assert (("Linear algebra", "Fundamentals", "quotient"), "") in t
    assert (("Linear algebra", "Duality", "dual vector space"), "Module.Dual") in t
    assert (("Topology", "Streams", "stream"), "Stream'") in t
    assert (("Topology", "Streams", "classification of abelian simple groups"), "CommGroup.is_simple_iff_prime_card") in t
    assert {(("Topology", "Streams", "two decls"), d) for d in ("A.one", "A.two")} <= t
    assert len(t) == 9


def test_ingest_adds_alias_and_provenance(tmp_path, monkeypatch):
    T = "2026-10-16T00:00:00+00:00"
    st = Store()
    st.add_source(Source("src:mathlib4", "Mathlib", "Apache-2.0"))
    p = Provenance("src:mathlib4", "Mathlib/X.lean:1", "atlas.ingest.mathlib", "1", "Apache-2.0", T)
    st.add_entity(Entity("mathlib:Module", EntityType.DEFINITION, (LangString("Module", "und"),), provenance=(p,)))
    monkeypatch.setattr(mt, "read_files", lambda path: {"docs/undergrad.yaml": YAML})
    snap = mt.Snapshot(tmp_path / "x.tar.gz", "abc", "lean4", T)
    rep = mt.ingest(st, snap)
    e = st.get_entity("mathlib:Module")
    assert ("en", "vector space") in {(a.lang, a.text) for a in e.aliases}
    prov = e.provenance[-1]
    assert prov.source_id == mt.SOURCE_ID and prov.locator == "docs/undergrad.yaml:4"
    assert prov.quote == "Linear algebra › Fundamentals › vector space: Module"
    assert st.get_source(mt.SOURCE_ID).kind == "library"
    assert rep.counters["decl_not_in_registry"] == 6 and rep.counters["docs/undergrad.yaml:not_formalized"] == 1
    assert len(list(st.events("mathlib:Module", kinds=[EventKind.ENTITY_REPLACED]))) == 1
    assert mt.ingest(st, snap).counters["declarations_linked"] == 0  # idempotent
    assert [x.entity_id for x in st.search("vector space")][:1] == ["mathlib:Module"]
