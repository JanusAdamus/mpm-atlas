"""Round-5 name curation rules (ADR-0005 §5): reversible rows in the registry's `curation` table.

Every rule signs as `curation:<rule>/<version>` and only goes through `Store.add_curation` /
`Store.undo_curation`; nothing is deleted and the source labels are never touched.

- `out-of-domain/1`: items that are not mathematics (chess, countries, physical quantities, units,
  software...) leave every presentation scope, and their homonymous memberships are retired.
  The list is the one of `docs/plans/r5/diagnostico-calidad.md` §1.2 plus the non-mathematical words
  of its Q4 list, read one by one with their Wikidata descriptions (agent judgment, 2026-10-19).
- `derived-out-of-general/1`: derived Mathlib zones (`concept:Mathlib/...`) leave `map:general`.
- `qualifier/1`: a qualified label for an ambiguous single word or a homonym, written from the item's
  own Wikidata description or English label (quoted in the provenance).
- `capitalization/1`: lower-case first letter when Wikidata's English label is lower case and the first
  Spanish word does not appear capitalized in the English label (a proper noun).
- `duplicate-label/1`: one merge PROPOSAL per non-survivor of each near-duplicate group (diagnóstico §1.1,
  35 groups).  Nothing is merged; the evidence carries the agent's judgment to speed up the decision.
- `homonym-membership/1`: memberships of a declaration in an item that only shares a word with it (`Disjoint` ->
  exclusive or, `sq` -> the quadrilateral), per HOMONYM_SENSES (judges r5, B5-04).
- `named-after/1`: P138 claims that overstate the source (pi «named after» Ludolph van Ceulen, C4-06).

`python -m atlas.ingest.curation apply [--db data/atlas.db]` runs them all (idempotent);
`python -m atlas.ingest.curation undo <rule> <version> --reason ...` undoes one rule.
"""

from __future__ import annotations

import argparse
import json
import sqlite3
import sys
import time
from collections import Counter
from typing import Callable, Iterable, Mapping, TypeVar

from ..model import CURRENT_STATES, CurationKind, Entity, EntityType, Provenance, RelationType, ValidationError
from ..store import Store
from .common import DEFAULT_CACHE

ALL_SCOPES = ["map:general", "map:all", "search:default", "concept_layer"]
NUMBER_SCOPES = ["map:general", "search:default"]
WD_LICENSE = "CC0-1.0"
T = TypeVar("T")

# qid -> reason.  Scopes: ALL_SCOPES unless listed in NUMBERS.
OUT_OF_DOMAIN: Mapping[str, str] = {
    # chess
    "Q1055911": "ajedrez", "Q1137224": "ajedrez", "Q137": "ajedrez", "Q20819848": "ajedrez", "Q215599": "ajedrez",
    "Q2590761": "ajedrez", "Q842046": "ajedrez", "Q970141": "ajedrez", "Q332225": "ajedrez", "Q3984344": "ajedrez",
    "Q828074": "ajedrez",
    # countries
    "Q810": "país", "Q35": "país", "Q30": "país", "Q145": "país", "Q258": "país",
    # physical quantities and phenomena, earth sciences, chemistry, units
    "Q11379": "física", "Q42213": "física", "Q589": "astronomía", "Q209082": "química", "Q83371": "sismología",
    "Q46255": "geofísica", "Q9128": "física", "Q4173969": "unidad", "Q304479": "unidad", "Q12142910": "física",
    "Q11412": "física", "Q11423": "física", "Q13222844": "física", "Q223557": "física", "Q3386703": "física",
    "Q208650": "ciencias naturales", "Q136913087": "física (marginal)",
    # computing artefacts, software and other non-mathematical words
    "Q186152": "informática", "Q858684": "informática", "Q817765": "informática", "Q4303335": "informática",
    "Q6509476": "software", "Q113289177": "software", "Q879717": "criptografía histórica",
    "Q1251376": "deporte", "Q41207": "economía", "Q170924": "economía", "Q42848": "información",
    "Q11028": "información", "Q6428674": "cronología", "Q193642": "cartografía", "Q3757616": "nombre de pila",
    # numbers: mathematics, but not concepts for the general map or the default search
    "Q4587955": "número",
}
NUMBERS = frozenset({"Q4587955"})

# entity -> (lang, text, basis): basis is the Wikidata field quoted as provenance ("description:en", "label:en"...)
QUALIFIERS: Mapping[str, tuple[str, str, str]] = {
    "wd:Q1096885": ("es", "red (subgrupo de un espacio vectorial real)", "description:en"),
    "wd:Q595298": ("es", "haz (sheaf)", "label:en"),
    "wd:Q5375781": ("es", "fin (transformación dinatural universal)", "description:en"),
    "wd:Q7595945": ("es", "campo (stack)", "label:en"),
    "wd:Q3064117": ("es", "cara (de un poliedro)", "description:es"),
    "wd:Q6265342": ("es", "signo más", "label:en"),
    "wd:Q1969983": ("es", "diferencial (pushforward)", "label:en"),
    "wd:Q1626444": ("es", "cut (teoría de grafos)", "description:en"),
    "wd:Q3828144": ("es", "carta (homeomorfismo de un espacio topológico)", "description:en"),
    "wd:Q2589168": ("es", "grafo estrella", "label:en"),
    "wd:Q2594083": ("es", "palabra (lenguajes formales)", "description:en"),
    "wd:Q12510": ("es", "toro (superficie de revolución)", "description:es"),
    "wd:Q838611": ("es", "bola (espacio al interior de una esfera)", "description:es"),
    "wd:Q118084": ("es", "unidad (elemento invertible de un anillo)", "description:en"),
    "wd:Q919107": ("es", "producto (teoría de categorías)", "description:es"),
    "wd:Q2361385": ("es", "dual (teoría de categorías)", "label:en"),
    "wd:Q204": ("es", "cero", "label:en"),
    "wd:Q116838634": ("es", "factor (álgebra de von Neumann)", "label:en"),
    "wd:Q1868414": ("es", "propiedad local", "label:en"),
    "wd:Q2532728": ("es", "complete (clase de complejidad)", "description:en"),
    "wd:Q3010804": ("es", "core (subgrupo normal)", "description:en"),
    "wd:Q834884": ("es", "type (teoría de modelos)", "description:en"),
    "wd:Q7111912": ("es", "outcome (teoría de juegos)", "description:en"),
    "wd:Q471771": ("es", "clave (criptografía)", "description:en"),
    "wd:Q4652392": ("es", "ALL (clase de complejidad)", "description:es"),
    "wd:Q105225393": ("es", "head (vértice de llegada de una arista dirigida)", "description:en"),
    "wd:Q105225381": ("es", "tail (vértice de salida de una arista dirigida)", "description:en"),
    # homonym groups of diagnóstico §1.2
    "wd:Q845080": ("es", "sección transversal", "label:en"),
    "wd:Q11703678": ("es", "sección (de un fibrado)", "description:en"),
    "wd:Q17103180": ("es", "sección (de un morfismo)", "description:en"),
    "wd:Q42989": ("es", "topología (rama de las matemáticas)", "description:en"),
    "wd:Q18786106": ("es", "topología (estructura de abiertos)", "description:en"),
    "wd:Q36253": ("es", "longitud (medida)", "description:en"),
    "wd:Q1761084": ("es", "longitud de un módulo", "description:en"),
    "wd:Q44528": ("es", "vector euclídeo", "label:en"),
    "wd:Q861555": ("es", "cubo (tercera potencia)", "description:en"),
    "wd:Q812880": ("es", "cubo (sólido)", "description:en"),
    "wd:Q44649": ("es", "ideal (de un anillo)", "description:en"),
    "wd:Q1656686": ("es", "ideal (de un orden)", "description:en"),
    "wd:Q484298": ("es", "superficie (variedad de dimensión dos)", "description:en"),
    "wd:Q3505239": ("es", "superficie (de un cuerpo)", "description:en"),
    "wd:Q2742711": ("es", "distancia en un grafo", "label:en"),
    "wd:Q1304193": ("es", "vértice (de un grafo)", "description:en"),
    "wd:Q26401": ("es", "vértice (de una figura geométrica)", "description:en"),
    "wd:Q3297804": ("es", "arista (de un grafo)", "description:en"),
    "wd:Q26382": ("es", "arista (de un polígono o politopo)", "description:en"),
    "wd:Q4720939": ("es", "clausura (bajo una operación)", "description:en"),
    "wd:Q320346": ("es", "clausura (topológica)", "description:en"),
    "wd:Q1142861": ("es", "suma directa de módulos", "label:en"),
    "wd:Q3836720": ("es", "bucle (estructura algebraica)", "description:en"),
    "wd:Q512604": ("es", "bucle (arista de un vértice a sí mismo)", "description:en"),
    "wd:Q1139344": ("es", "pendiente (de una superficie)", "description:en"),
    "wd:Q275447": ("es", "pendiente (de una recta)", "description:en"),
    "wd:Q1397439": ("es", "pullback (teoría de categorías)", "description:en"),
    "wd:Q25345330": ("es", "pullback (en cohomología)", "description:en"),
}

# (survivor, merged, judgment) for the 35 near-duplicate groups of diagnóstico §1.1 (Q2).
DUPLICATE_GROUPS: tuple[tuple[str, ...], ...] = (
    ("wd:Q845080", "wd:Q11703678", "wd:Q17103180"), ("wd:Q42989", "wd:Q18786106"), ("wd:Q36253", "wd:Q1761084"),
    ("wd:Q44528", "wd:Q186152"), ("wd:Q812880", "wd:Q861555"), ("wd:Q44649", "wd:Q1656686"),
    ("wd:Q484298", "wd:Q3505239"), ("wd:Q126017", "wd:Q2742711"), ("wd:Q26401", "wd:Q1304193"),
    ("wd:Q26382", "wd:Q3297804"), ("wd:Q320346", "wd:Q4720939"), ("wd:Q13582243", "wd:Q1142861"),
    ("wd:Q3836720", "wd:Q512604"), ("wd:Q202805", "wd:Q970141"), ("wd:Q105225381", "wd:Q140092311"),
    ("wd:Q105225393", "wd:Q140092320"), ("wd:Q275447", "wd:Q1139344"), ("wd:Q1397439", "wd:Q25345330"),
    ("wd:Q60794278", "wd:Q25098585"),
) + tuple((f"concept:Mathlib/{a}", f"concept:Mathlib/{b}") for a, b in (
    ("Algebra/Algebra/Equiv", "Topology/Algebra/Algebra/Equiv"),
    ("Algebra/Algebra/Unitization", "Analysis/Normed/Algebra/Unitization"),
    ("Algebra/BigOperators/Group/List", "Algebra/Order/BigOperators/Group/List"),
    ("Algebra/BigOperators/GroupWithZero", "Algebra/Order/BigOperators/GroupWithZero"),
    ("Algebra/BigOperators/Ring", "Algebra/Order/BigOperators/Ring"),
    ("Algebra/Group/Action", "Algebra/Order/Group/Action"),
    ("Algebra/Group/Equiv", "Algebra/Order/Group/Equiv"),
    ("Algebra/Group/Opposite", "Algebra/Order/Group/Opposite"),
    ("Algebra/Module", "Topology/Algebra/Module"),
    ("Algebra/Module/Equiv", "Topology/Algebra/Module/Equiv"),
    ("Algebra/TrivSqZeroExt", "Analysis/Normed/Algebra/TrivSqZeroExt"),
    ("Analysis/Normed/Module/FiniteDimension", "Topology/Algebra/Module/FiniteDimension"),
    ("LinearAlgebra/Alternating/Uncurry", "Analysis/Normed/Module/Alternating/Uncurry"),
    ("Data/Fin/Tuple", "Algebra/Group/Fin/Tuple"),
    ("Order/Category", "Topology/Order/Category"),
    ("CategoryTheory/Monoidal/Closed", "Algebra/Category/ModuleCat/Monoidal/Closed"),
))
# groups the agent reads as the same thing twice; every other group is distinct senses (qualify instead)
PROBABLE_DUPLICATES = frozenset({"wd:Q140092311", "wd:Q140092320", "wd:Q25098585"})
RETIRED_RELATIONS = (RelationType.MEMBER_OF_CONCEPT,)

# homonym-membership/1 (B5-04, judges r5): a Mathlib word read as a Wikidata item whose sense is another one.
# qid -> (reason, keep(name, module) or None = retire every non-curated membership, out of the concept layer?).
# `True` in the third field: the item's sense is not mathematics (a unit, physics, music...), so it also leaves the
# concept_layer and a re-run of the projection gives it no members.  Read one by one from the item's Wikidata
# description and a sample of its members (agent judgment, 2026-10-20); docs/plans/r5/homonimos-pendientes.tsv
# lists what the same sweep caught and was not applied.
Keep = Callable[[str, str], bool]
_CAT = ("Mathlib/CategoryTheory/", "Mathlib/Algebra/Homology/", "Mathlib/AlgebraicTopology/", "Mathlib/Algebra/Category/")
HOMONYM_SENSES: Mapping[str, tuple[str, Keep | None, bool]] = {
    # the judges' cases
    "Q498186": ("`Disjoint` (conjuntos disjuntos, ínfimo ⊥) no es la disyunción exclusiva",
                lambda n, m: "isjoint" not in n.lower() or "xor" in n.lower(), False),
    "Q164": ("`sq`/`IsSquare`/`CommSq` (elevar al cuadrado, cuadrado conmutativo) no es el cuadrilátero", None, False),
    "Q19821": ("triángulo distinguido, identidades triangulares o 2-símplice, no el polígono",
               lambda n, m: not m.startswith(_CAT), False),
    "Q65090820": ("«correspondence» (de Galois, biyección) no es la correspondencia entre variedades", None, False),
    "Q6086107": ("`IsIso`/`Iso` es un morfismo invertible, no un isomorfismo entre categorías",
                 lambda n, m: "IsoCat" in m or "/Category/Cat/" in m, False),
    # same pattern in the judges' samples
    "Q4386723": ("conjunto de índices de una familia, no el index set de la computabilidad", None, False),
    "Q948010": ("`IsPrimitiveRoot` es raíz primitiva de la unidad, no raíz primitiva módulo n",
                lambda n, m: "LegendreSymbol" in m, False),
    "Q4440864": ("dimensión inyectiva o proyectiva (homológica), no la dimensión",
                 lambda n, m: not any(k in n.lower() for k in ("injectivedimension", "projectivedimension")), False),
    # the same rule, found by the sweep (one-word match to an item without a domain guard)
    "Q42344": ("cono de un funtor (límites), no la figura geométrica", lambda n, m: not m.startswith(_CAT), False),
    "Q17504209": ("`star` es la involución estrella, no el polígono estrellado", None, False),
    "Q5375781": ("`Fin n` y `End` (endomorfismos), no el end de un funtor",
                 lambda n, m: "Limits/Shapes/End" in m, False),
    "Q1165839": ("`range` es la imagen de una función, no el rango estadístico", None, False),
    "Q7271381": ("«functor» de Mathlib, no una consulta entre estructuras", None, False),
    "Q3345662": ("núcleo de Markov, de una aplicación o reproductor, no la ventana estadística", None, False),
    "Q4652392": ("la palabra «all», no la clase de complejidad ALL", None, False),
    "Q116838634": ("`factor` (factorización), no el factor de un álgebra de von Neumann", None, False),
    "Q617388": ("«little» de o pequeña, no la ley de Little", None, False),
    "Q1816844": ("«big» de O grande, no un número grande", None, False),
    "Q1327058": ("la palabra «third», no la ley del tercero", None, False),
    "Q2257676": ("«standard» (étale estándar, símplice estándar), no la aplicación estándar caótica", None, False),
    "Q1062971": ("el alias «auxiliary», no la ecuación característica", None, False),
    "Q3001212": ("`lift` de Mathlib, no el lift de la minería de datos", None, True),
    "Q376660": ("«natural» (`Nat`, transformación natural), no la unidad de información", None, True),
    "Q2141130": ("continuidad matemática, no la continuidad narrativa", None, True),
    "Q1139519": ("flecha de una categoría (`Arrow`), no el símbolo de dirección", None, True),
    "Q28390": ("grado de un polinomio o de un vértice, no la unidad de ángulo", None, True),
    "Q1": ("universo de tipos de Lean, no el universo físico", None, True),
    "Q902301": ("multiplicidad de una raíz o de un divisor, no la de un nivel cuántico", None, True),
    "Q2642727": ("periodo de una función o de una acción, no la unidad temporal", None, True),
    "Q636099": ("paso de una máquina o de una construcción, no el intervalo musical", None, True),
    "Q500515": ("`Gal` (grupo de Galois), no la unidad de aceleración", None, True),
    "Q203872": ("la palabra «being», no el ser de la filosofía", None, True),
    "Q902090": ("`Cᵒᵖ` (categoría opuesta), no el coeficiente de rendimiento", None, True),
    "Q7964839": ("acción de un grupo o `Action`, no la carga estructural", None, True),
    "Q5150188": ("columna de una matriz, no la de un almacén NoSQL", None, True),
    "Q1110301": ("la palabra «transform», no la transformación Y-Δ de circuitos", None, True),
    "Q186386": ("proyección de un producto, no la proyección cartográfica", None, True),
    "Q45003": ("entropía topológica o de la información, no la magnitud física", None, True),
    "Q4130": ("órbita de una acción o de una función, no la trayectoria de un cuerpo", None, True),
    "Q468777": ("la palabra «existence», no la existencia de la filosofía", None, True),
    "Q32032": ("red (topología, recubrimientos), no el importe neto", None, True),
    "Q829073": ("el alias «second» (segunda componente), no el segundo de arco", None, True),
    "Q877977": ("variable de un polinomio o de una fórmula, no la variable de un programa", None, True),
}


def actor(rule: str, version: str) -> str:
    return f"curation:{rule}/{version}"


def _retry(fn: Callable[[], T], tries: int = 60) -> T:
    """Another worker writes to the same registry: wait for the lock instead of failing."""
    for k in range(tries):
        try:
            return fn()
        except sqlite3.OperationalError as exc:
            if "locked" not in str(exc) and "busy" not in str(exc) or k == tries - 1:
                raise
            time.sleep(min(2.0, 0.1 * (k + 1)))
    raise AssertionError("unreachable")


def _wd_prov(e: Entity, basis: str, rule: str, version: str) -> Provenance:
    field, lang = basis.split(":")
    texts = {l.lang: l.text for l in (e.descriptions if field == "description" else e.labels)}
    if lang not in texts:
        raise ValidationError(f"{e.id}: no {basis} to support the curation")
    base = next((p for p in e.provenance if p.source_id == "src:wikidata"), None)
    if base is None:
        raise ValidationError(f"{e.id}: no Wikidata provenance")
    return Provenance("src:wikidata", f"https://www.wikidata.org/wiki/{e.id[3:]}#{field}-{lang}", f"curation:{rule}",
                      version, WD_LICENSE, base.retrieved_at, quote=texts[lang])


def _current_claims(store: Store, entity_id: str, role: str, relations: Iterable[RelationType],
                    states: Iterable[str] | None = None) -> list[str]:
    rels = [r.value for r in relations]
    st = sorted(states or {s.value for s in CURRENT_STATES})
    q = (f"SELECT c.id FROM claim_participants p JOIN claims c ON c.id=p.claim_id WHERE p.entity_id=? AND p.role=? "
         f"AND c.relation IN ({','.join('?' * len(rels))}) AND c.evidence_state IN ({','.join('?' * len(st))}) "
         "ORDER BY c.id")
    return [r[0] for r in store._conn.execute(q, (entity_id, role, *rels, *st))]


def out_of_domain(store: Store, version: str = "1") -> Counter:
    """Exclude every listed item that is in the registry and retire its homonymous claims: every current
    membership that makes it a concept, and `formalized_as` proposed only by name matching."""
    rule, who, n = "out-of-domain", actor("out-of-domain", version), Counter()
    for qid, why in OUT_OF_DOMAIN.items():
        eid = "wd:" + qid
        if store.get_entity(eid, resolve=False) is None:
            n["missing"] += 1
            continue
        scopes = NUMBER_SCOPES if qid in NUMBERS else ALL_SCOPES
        _retry(lambda: store.add_curation(CurationKind.EXCLUDE_PRESENTATION, eid, rule=rule, rule_version=version,
                                          payload={"scopes": scopes}, actor=who,
                                          reason=f"fuera de dominio ({why}); diagnóstico r5 §1.2"))
        n["excluded"] += 1
        if qid in NUMBERS:
            continue
        if store.curations(target_id=eid, kind=CurationKind.RETIRE_CLAIMS):
            n["retire_already_active"] += 1
            continue
        claims = (_current_claims(store, eid, "object", RETIRED_RELATIONS)
                  + _current_claims(store, eid, "subject", (RelationType.FORMALIZED_AS,), ("model_proposed",)))
        if not claims:
            continue
        _retry(lambda: store.add_curation(CurationKind.RETIRE_CLAIMS, eid, rule=rule, rule_version=version,
                                          payload={"claims": claims}, actor=who,
                                          reason=f"pertenencias homónimas de un ítem fuera de dominio ({why})"))
        n["retire_rows"] += 1
        n["claims_retired"] += len(claims)
    return n


def _memberships(store: Store, item: str) -> list[tuple[str, str, str]]:
    """Current non-curated memberships of `item`: (claim id, declaration name, Mathlib module)."""
    st = sorted(s.value for s in CURRENT_STATES)
    q = ("SELECT c.id, json_extract(c.data,'$.provenance[0].quote'), e.id, e.data FROM claim_participants p "
         "JOIN claims c ON c.id=p.claim_id JOIN claim_participants s ON s.claim_id=c.id AND s.role='subject' "
         "JOIN entities e ON e.id=s.entity_id WHERE p.entity_id=? AND p.role='object' AND c.relation=? "
         f"AND c.evidence_state IN ({','.join('?' * len(st))}) ORDER BY c.id")
    out = []
    for cid, quote, did, data in store._conn.execute(q, (item, RelationType.MEMBER_OF_CONCEPT.value, *st)):
        if json.loads(quote or "{}").get("signal") == "curated":
            continue
        d = json.loads(data)
        name = next((v for s, v in d.get("external_ids") or () if s == "mathlib"), did.removeprefix("mathlib:"))
        loc = ((d.get("provenance") or [{}])[0]).get("locator", "")
        out.append((cid, name, loc.split(".lean", 1)[0]))
    return out


def homonym_memberships(store: Store, version: str = "1") -> Counter:
    """B5-04: retire the memberships of every HOMONYM_SENSES item that its keep rule does not keep, one
    retire_claims row per item (undo restores them); items whose sense is not mathematics also leave the layer."""
    rule, who, n = "homonym-membership", actor("homonym-membership", version), Counter()
    for qid, (why, keep, out_of_layer) in HOMONYM_SENSES.items():
        eid = "wd:" + qid
        if store.get_entity(eid, resolve=False) is None:
            n["missing"] += 1
            continue
        if out_of_layer:
            _retry(lambda: store.add_curation(CurationKind.EXCLUDE_PRESENTATION, eid, rule=rule, rule_version=version,
                                              payload={"scopes": ["concept_layer"]}, actor=who,
                                              reason=f"homónimo fuera de las matemáticas: {why}"))
            n["out_of_layer"] += 1
        if any(c.rule == rule for c in store.curations(target_id=eid, kind=CurationKind.RETIRE_CLAIMS)):
            n["retire_already_active"] += 1
            continue
        rows = _memberships(store, eid)
        claims = [cid for cid, name, module in rows if keep is None or not keep(name, module)]
        n["kept"] += len(rows) - len(claims)
        if not claims:
            continue
        _retry(lambda: store.add_curation(CurationKind.RETIRE_CLAIMS, eid, rule=rule, rule_version=version,
                                          payload={"claims": claims}, actor=who, reason=f"pertenencias homónimas: {why}"))
        n["retire_rows"] += 1
        n["claims_retired"] += len(claims)
    return n


# named-after/1 (C4-06): a P138 claim that says more than the source supports.
NAMED_AFTER_RETIRED: Mapping[str, str] = {
    "c:c77bbb29d006dccd": "pi no lleva el nombre de Ludolph van Ceulen: en alemán se le llamó «número ludolfino» "
                          "(Ludolphsche Zahl) por su cálculo de 35 decimales; Wikidata Q167#P138 lo reduce a «named "
                          "after».  Se retira hasta que una fuente historiográfica respalde la frase exacta.",
}


def named_after(store: Store, version: str = "1") -> Counter:
    rule, who, n = "named-after", actor("named-after", version), Counter()
    for claim_id, why in NAMED_AFTER_RETIRED.items():
        c = store.get_claim(claim_id)
        if c is None or c.evidence_state not in CURRENT_STATES:
            n["not_current"] += 1
            continue
        _retry(lambda: store.add_curation(CurationKind.RETIRE_CLAIMS, c.subject, rule=rule, rule_version=version,
                                          payload={"claims": [claim_id]}, actor=who, reason=why))
        n["claims_retired"] += 1
    return n


def derived_out_of_general(store: Store, version: str = "1") -> Counter:
    rule, who, n = "derived-out-of-general", actor("derived-out-of-general", version), Counter()
    ids = [r[0] for r in store._conn.execute(
        "SELECT id FROM entities WHERE id LIKE 'concept:Mathlib/%' AND type='concept' AND merged_into IS NULL ORDER BY id")]
    for eid in ids:
        _retry(lambda: store.add_curation(CurationKind.EXCLUDE_PRESENTATION, eid, rule=rule, rule_version=version,
                                          payload={"scopes": ["map:general"]}, actor=who,
                                          reason="zona derivada de Mathlib: no es un concepto principiante (r5 §1.4)"))
        n["excluded"] += 1
    return n


def _label(store: Store, eid: str, lang: str, text: str, prov: Provenance, rule: str, version: str,
           reason: str) -> bool:
    """Active label of the same rule with another text is undone first (changing = undo + create)."""
    who = actor(rule, version)
    payload = {"lang": lang, "text": text, "provenance": dict(prov.__dict__)}
    for cur in store.curations(target_id=eid, kind=CurationKind.PRESENTATION_LABEL):
        if cur.rule == rule and cur.rule_version == version and cur.payload.get("text") != text:
            _retry(lambda: store.undo_curation(cur.id, actor=who, reason="rótulo reemplazado"))
    before = store.curations(target_id=eid, kind=CurationKind.PRESENTATION_LABEL)
    _retry(lambda: store.add_curation(CurationKind.PRESENTATION_LABEL, eid, rule=rule, rule_version=version,
                                      payload=payload, actor=who, reason=reason))
    return len(store.curations(target_id=eid, kind=CurationKind.PRESENTATION_LABEL)) > len(before)


def qualifiers(store: Store, version: str = "1") -> Counter:
    n = Counter()
    for eid, (lang, text, basis) in QUALIFIERS.items():
        e = store.get_entity(eid, resolve=False)
        if e is None:
            n["missing"] += 1
            continue
        prov = _wd_prov(e, basis, "qualifier", version)
        n["created" if _label(store, eid, lang, text, prov, "qualifier", version,
                              f"calificador de palabra ambigua u homónimo, desde {basis} de Wikidata") else "kept"] += 1
    return n


def lower_initial(es: str, en: str) -> str | None:
    """Lower-cased ES label, or None when the rule does not apply (see module doc)."""
    first = es.split()[0] if es.split() else ""
    if not first or not first[0].isupper() or first.isupper() or not en[:1].islower():
        return None
    if any(w == first for w in en.replace("'s", " ").split()):  # appears capitalized in EN: a proper noun
        return None
    return es[0].lower() + es[1:]


def capitalization(store: Store, version: str = "1") -> Counter:
    n = Counter()
    rows = store._conn.execute(
        "SELECT id, data FROM entities WHERE id LIKE 'wd:%' AND merged_into IS NULL "
        "AND type IN ('concept','area','statement','definition','theory','algorithm','model','conjecture') "
        "AND json_extract(data,'$.labels.es') IS NOT NULL AND json_extract(data,'$.labels.en') IS NOT NULL").fetchall()
    for eid, data in rows:
        if eid in QUALIFIERS:
            continue
        d = json.loads(data)
        text = lower_initial(d["labels"]["es"], d["labels"]["en"])
        if text is None:
            continue
        e = Entity.from_json(d)
        prov = _wd_prov(e, "label:en", "capitalization", version)
        n["created" if _label(store, eid, "es", text, prov, "capitalization", version,
                              "mayúscula inicial copiada del título; la etiqueta EN va en minúscula") else "kept"] += 1
    return n


def duplicate_proposals(store: Store, version: str = "1") -> Counter:
    rule, who, n = "duplicate-label", actor("duplicate-label", version), Counter()
    for group in DUPLICATE_GROUPS:
        survivor, *others = group
        for merged in others:
            if store.get_entity(survivor, resolve=False) is None or store.get_entity(merged, resolve=False) is None:
                n["missing"] += 1
                continue
            judgment = "probable_duplicate" if merged in PROBABLE_DUPLICATES else "distinct_senses_qualify_instead"
            ev = json.dumps({"rule": "normalized ES label equal (diagnóstico r5 Q2)", "group": list(group),
                             "agent_judgment": judgment}, ensure_ascii=False)
            _retry(lambda: store.add_curation(CurationKind.PROPOSE_MERGE, merged, rule=rule, rule_version=version,
                                              payload={"survivor": survivor, "merged": merged, "evidence": ev},
                                              actor=who, reason=f"misma etiqueta normalizada; juicio de agente: {judgment}"))
            n[judgment] += 1
    return n


RULES: Mapping[str, Callable[[Store, str], Counter]] = {
    "out-of-domain": out_of_domain, "derived-out-of-general": derived_out_of_general, "qualifier": qualifiers,
    "capitalization": capitalization, "duplicate-label": duplicate_proposals,
    "homonym-membership": homonym_memberships, "named-after": named_after,
}


def apply_all(store: Store, version: str = "1") -> dict[str, dict[str, int]]:
    return {name: dict(fn(store, version)) for name, fn in RULES.items()}


def undo_rule(store: Store, rule: str, version: str, *, reason: str) -> int:
    rows = [c for c in store.curations() if c.rule == rule and c.rule_version == version]
    for c in rows:
        _retry(lambda: store.undo_curation(c.id, actor=actor(rule, version), reason=reason))
    return len(rows)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="python -m atlas.ingest.curation")
    ap.add_argument("action", choices=["apply", "undo"])
    ap.add_argument("rule", nargs="?")
    ap.add_argument("version", nargs="?", default="1")
    ap.add_argument("--db", default=str(DEFAULT_CACHE.parent / "atlas.db"))
    ap.add_argument("--reason", default="")
    args = ap.parse_args(argv)
    with Store(args.db) as store:
        store._conn.execute("PRAGMA busy_timeout = 60000")
        if args.action == "apply":
            out: object = apply_all(store) if args.rule is None else dict(RULES[args.rule](store, args.version))
        else:
            out = {"undone": undo_rule(store, args.rule, args.version, reason=args.reason)}
    sys.stdout.reconfigure(encoding="utf-8")
    print(json.dumps(out, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
