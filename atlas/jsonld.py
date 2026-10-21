"""JSON-LD export helpers and the pure-Python twin of shapes/atlas.shacl.ttl (docs/SPEC.md section 13).

Split out of atlas/store.py (2026-10-19); the architect owns both.  `Store.export_jsonld` builds the graph
with these helpers; `validate_jsonld` checks an exported graph without rdflib.
"""

from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Any, Iterable, Mapping
from urllib.parse import quote

from .model import RELATIONS, Claim, DateKind, Entity, EvidenceState, RelationType, Role, ValidationError, parse_edtf

CONTEXT_PATH = Path(__file__).resolve().parent.parent / "shapes" / "context.jsonld"
SHACL_PATH = Path(__file__).resolve().parent.parent / "shapes" / "atlas.shacl.ttl"


def _iri(atlas_id: str) -> str:
    return "aid:" + quote(atlas_id, safe="")


def _lang_map(items: Iterable[Any]) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for x in items:
        out.setdefault(x.lang, []).append(x.text)
    return {k: v[0] if len(v) == 1 else v for k, v in out.items()}


def _entity_ld(e: Entity, merged_from: list[str]) -> dict[str, Any]:
    node: dict[str, Any] = {"@id": _iri(e.id), "@type": "".join(w.capitalize() for w in e.type.value.split("_")),
                            "atlasId": e.id, "label": _lang_map(e.labels)}
    if e.aliases:
        node["altLabel"] = _lang_map(e.aliases)
    if e.descriptions:
        node["description"] = _lang_map(e.descriptions)
    if e.statements:
        node["statementText"] = _lang_map(e.statements)
    if e.formula:
        node["formula"] = e.formula
    if e.external_ids:
        node["externalId"] = [f"{s}:{v}" for s, v in e.external_ids]
    if merged_from:
        node["mergedFrom"] = [_iri(m) for m in merged_from]
    if e.provenance:
        node["provenance"] = [_prov_ld(p) for p in e.provenance]
    if e.dates:
        node["date"] = [{"dateKind": d.kind.value, "edtf": d.edtf, "calendar": d.calendar,
                         "provenance": [_prov_ld(d.provenance)], **({"raw": d.raw} if d.raw else {})}
                        for d in e.dates]
    if e.note is not None:
        node["noteSection"], node["noteOrdinal"] = e.note.section.value, e.note.ordinal
    return node


def _prov_ld(p: Any) -> dict[str, Any]:
    node = {"source": _iri(p.source_id), "locator": p.locator, "extractor": p.extractor,
            "extractorVersion": p.extractor_version, "license": p.license, "retrievedAt": p.retrieved_at}
    if p.quote:
        node["quote"] = p.quote
    return node


def _claim_ld(c: Claim, resolve: Any) -> dict[str, Any]:
    node: dict[str, Any] = {
        "@id": _iri(c.id), "@type": "Claim", "atlasId": c.id, "relation": c.relation.value,
        "evidenceState": c.evidence_state.value,
        "participant": [{"role": p.role.value, "entity": _iri(resolve(p.entity_id))} for p in c.participants],
        "provenance": [_prov_ld(p) for p in c.provenance],
    }
    optional = {
        "assumption": list(c.assumptions) or None, "validityDomain": c.validity_domain,
        "alternativeGroup": c.alternative_group, "supersedes": c.supersedes and _iri(c.supersedes),
        "supersededBy": c.superseded_by and _iri(c.superseded_by),
        "formalArtifact": c.formal and {"library": c.formal.library, "version": c.formal.version,
                                        "declaration": c.formal.declaration, "axiom": list(c.formal.axioms)},
        "confidence": c.confidence and {"value": c.confidence.value, "estimates": c.confidence.estimates,
                                        "calibration": c.confidence.calibration},
        "historicalDate": c.historical_date,
        "explanation": _lang_map(c.explanation) or None,
        "disagreement": list(c.disagreements) or None,
    }
    node.update({k: v for k, v in optional.items() if v})
    return node


# ---------------------------------------------------------------------- JSON-LD validator (mirrors SHACL)

_PROV_FIELDS = ("source", "locator", "extractor", "extractorVersion", "license", "retrievedAt")


def validate_jsonld(doc: Mapping[str, Any]) -> list[str]:
    """Pure-Python check of an exported graph against the rules in shapes/atlas.shacl.ttl.
    Returns a list of violations (empty = conforms)."""
    errors: list[str] = []
    nodes = doc.get("@graph", [])
    ids = {n.get("@id") for n in nodes}
    rel_values = {r.value for r in RelationType}
    state_values = {s.value for s in EvidenceState}
    for n in nodes:
        if n.get("@type") != "Claim":
            errors += _entity_ld_errors(n)
            continue
        cid = n.get("@id")
        rel, state = n.get("relation"), n.get("evidenceState")
        if rel not in rel_values:
            errors.append(f"{cid}: relation {rel!r} not in vocabulary")
        if state not in state_values:
            errors.append(f"{cid}: evidenceState {state!r} not in vocabulary")
        parts = n.get("participant", [])
        roles = [p.get("role") for p in parts]
        if roles.count("subject") != 1:
            errors.append(f"{cid}: needs exactly one subject participant")
        n_obj, n_prem = roles.count("object"), roles.count("premise")
        if n_prem and (n_obj or rel not in rel_values or not RELATIONS[RelationType(rel)].allows_premises):
            errors.append(f"{cid}: premises only without object and on relations that allow them")
        if not n_prem and n_obj != 1:
            errors.append(f"{cid}: needs exactly one object or a premise set")
        for p in parts:
            if p.get("role") not in {r.value for r in Role}:
                errors.append(f"{cid}: unknown role {p.get('role')!r}")
            if not isinstance(p.get("entity"), str) or p["entity"] not in ids:
                errors.append(f"{cid}: participant entity {p.get('entity')!r} missing from graph")
        provs = n.get("provenance", [])
        if not provs:
            errors.append(f"{cid}: no provenance")
        for p in provs:
            for f in _PROV_FIELDS:
                if not isinstance(p.get(f), str) or not p[f].strip():
                    errors.append(f"{cid}: provenance.{f} missing")
            try:
                datetime.fromisoformat(p.get("retrievedAt", ""))
            except (TypeError, ValueError):
                errors.append(f"{cid}: provenance.retrievedAt is not a dateTime")
        if state == "formally_linked":
            fa = n.get("formalArtifact") or {}
            if not all(fa.get(k) for k in ("library", "version", "declaration")):
                errors.append(f"{cid}: formally_linked requires formalArtifact(library, version, declaration)")
        if state == "formally_linked" and rel in rel_values and not RELATIONS[RelationType(rel)].formal_allowed:
            errors.append(f"{cid}: relation {rel} cannot be formally_linked")
        if n.get("historicalDate") is not None and not _edtf_ok(n["historicalDate"]):
            errors.append(f"{cid}: historicalDate is not EDTF of the supported subset")
        if state == "superseded" and not n.get("supersededBy"):
            errors.append(f"{cid}: superseded requires supersededBy")
        conf = n.get("confidence")
        if conf is not None and not (isinstance(conf.get("value"), (int, float)) and 0 <= conf["value"] <= 1
                                     and conf.get("estimates") and conf.get("calibration")):
            errors.append(f"{cid}: confidence needs value in [0,1], estimates and calibration")
    return errors


def _edtf_ok(text: Any, calendar: str = "gregorian") -> bool:
    try:
        parse_edtf(text, calendar)
    except ValidationError:
        return False
    return True


def _entity_ld_errors(n: Mapping[str, Any]) -> list[str]:
    nid, errors = n.get("@id"), []
    if n.get("@type") == "Source":
        return errors
    if not n.get("label"):
        errors.append(f"{nid}: entity without label")
    if (n.get("@type") == "Note") != ("noteSection" in n):
        errors.append(f"{nid}: noteSection is required for, and only for, notes")
    for d in n.get("date", []):
        if d.get("dateKind") not in {k.value for k in DateKind} or len(d.get("provenance", [])) != 1:
            errors.append(f"{nid}: a date needs a dateKind and exactly one provenance")
        if not _edtf_ok(d.get("edtf"), d.get("calendar", "gregorian")):
            errors.append(f"{nid}: date {d.get('edtf')!r} is not EDTF of the supported subset")
    return errors
