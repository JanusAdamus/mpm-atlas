"""r5: PlanetMath metadata, offline from the filtered cache; 'related' is never a prerequisite."""

import gzip
import json

from atlas import Entity, EntityType, EvidenceState, LangString, Provenance, RelationType, Source, Store
from atlas.ingest import planetmath as pm
from atlas.ingest.http import Response

T = "2026-10-16T00:00:00+00:00"
WD = Provenance("src:wikidata", "https://www.wikidata.org/wiki/Q1", "atlas.ingest.wikidata", "wd-map/2", "CC0-1.0", T)
TEX = r"""\documentclass[12pt]{article}
\pmcanonicalname{Bijection}
\pmtitle{bijection}
\pmtype{Definition}
\pmclassification{msc}{03-00}
\pmsynonym{bijective function}{Bijection}
\pmsynonym{one-to-one correspondence}{Bijection}
%\pmsynonym{commented out}{Bijection}
\pmdefines{permutation}
\pmrelated{Function}
\pmrelated{Nowhere}
\endmetadata
\pmrelated{InTheBody}
"""


def test_parse_header_reads_only_the_metadata():
    e = pm.parse_header(TEX, "03-00-Bijection.tex")
    assert e.canonical == "Bijection" and e.title == "bijection" and e.msc == ("03-00",)
    assert e.synonyms == ("bijective function", "one-to-one correspondence")
    assert e.defines == ("permutation",) and e.related == ("Function", "Nowhere")


class FakeClient:
    offline = True

    def __init__(self, cache_dir):
        self.cache_dir = cache_dir

    def get(self, url, params=None, **kw):
        return Response(json.dumps([{"name": "03_Logic", "default_branch": "master"},
                                    {"name": "fem2016", "default_branch": "master"}]).encode(), T, True)


def test_ingest_links_by_planetmath_id_and_types_related_as_candidate_citation(tmp_path):
    fn = pm.PMEntry("Function", "function", "Definition", ("03-00",), (), (), ("Bijection",), "03-00-Function.tex")
    with gzip.open(tmp_path / "03_Logic@abc123.jsonl.gz", "wt", encoding="utf-8") as fh:
        for e in (pm.parse_header(TEX, "03-00-Bijection.tex"), fn,
                  pm.PMEntry("Orphan", "orphan", "", (), ("x",), (), (), "o.tex")):
            fh.write(json.dumps(e.__dict__) + "\n")
    st = Store()
    st.add_source(Source("src:wikidata", "Wikidata", "CC0-1.0"))
    st.add_entities([Entity("wd:Q1", EntityType.CONCEPT, (LangString("bijection", "en"),),
                            external_ids=(("planetmath", "Bijection"),), provenance=(WD,)),
                     Entity("wd:Q2", EntityType.CONCEPT, (LangString("function", "en"),),
                            external_ids=(("planetmath", "Function"),), provenance=(WD,))])
    rep = pm.ingest(st, FakeClient(tmp_path))
    assert rep.counters["entries_read"] == 3 and rep.counters["entries_without_item"] == 1
    frag = st.get_entity("pm:Bijection")
    assert frag.type is EntityType.SOURCE_FRAGMENT and "abc123" in frag.provenance[0].locator
    assert frag.provenance[0].license == "CC-BY-SA-3.0" and st.get_source("src:planetmath").kind == "encyclopedia"
    assert not frag.statements  # the article text is not stored
    item = st.get_entity("wd:Q1")
    assert ("en", "one-to-one correspondence") in {(a.lang, a.text) for a in item.aliases}
    cites = list(st.iter_claims(RelationType.CITES))
    assert len(cites) == 2 and all(c.evidence_state is EvidenceState.MODEL_PROPOSED for c in cites)
    assert not list(st.iter_claims(RelationType.PREREQUISITE_FOR))
    assert {c.subject for c in st.iter_claims(RelationType.APPEARS_IN)} == {"wd:Q1", "wd:Q2"}
    again = pm.ingest(st, FakeClient(tmp_path))  # idempotent
    assert again.counters["claims_written"] == 0 and again.counters["items_with_new_aliases"] == 0
