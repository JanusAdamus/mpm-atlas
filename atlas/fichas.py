"""Fichas (reader's cards) as derived views of the registry (ADR-0005 §6, plan r5 point 3).

One sentence = one `note` entity + one `explains` claim (note -> concept) with its own provenance:
source, locator, extractor and version, licence, retrieval date and the supporting `quote`.
Sentences enter as `model_proposed`; only a person makes them `editorially_reviewed` (`review`).

Editorial input is markdown (`FORMAT` below).  `import_ficha` splits it into notes and claims; once
imported the registry rules.  `render_ficha` writes the markdown view back: reviewed sentences only,
unless `include_drafts`, and then the rest under «Borrador editorial, sin revisar».  «Qué saber antes»
and «Con qué se conecta» are not notes: they list existing claims and cite their `claim_id`.

    python -m atlas fichas import FILE.md... [--dry-run]   markdown -> notes + explains (model_proposed)
    python -m atlas fichas render ID... [--drafts]          registry -> data/fichas/<id>.md
    python -m atlas fichas review CLAIM... --reviewer NAME  a person marks sentences reviewed
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import unicodedata
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlparse

from .ingest import wikipedia as wp
from .model import (
    CURRENT_STATES,
    Claim,
    Entity,
    EntityType,
    EvidenceState,
    FichaSection,
    LangString,
    NoteInfo,
    Provenance,
    RelationType,
    Role,
    ValidationError,
    binary,
    now_iso,
    stable_id,
)
from .store import Store

FORMAT = "ficha/1"
DRAFT_LABEL = "Borrador editorial, sin revisar"
HEADINGS = {
    "qué es": FichaSection.QUE_ES,
    "por qué importa": FichaSection.POR_QUE_IMPORTA,
    "cómo compruebo que lo entendí": FichaSection.COMPRUEBO,
}
TITLES = {s: h[0].upper() + h[1:] for h, s in HEADINGS.items()}
AUTHORS = {"draft-llm/1", "editorial/1"}  # a model proposes; a person writes
KEYS = {"fuente": "source_id", "localizador": "locator", "cita": "quote", "recuperado": "retrieved_at"}
# «Qué saber antes»: the same claims the panel uses (web/src/panel.ts `prerequisitesOf`).
BEFORE = {RelationType.PREREQUISITE_FOR: Role.OBJECT, RelationType.DEPENDS_ON_LOGICALLY: Role.SUBJECT}
CONNECT = (RelationType.GENERALIZES, RelationType.SPECIAL_CASE_OF, RelationType.INSTANCE_OF,
           RelationType.EQUIVALENT_TO, RelationType.SAID_EQUIVALENT_TO, RelationType.ANALOGOUS_TO,
           RelationType.APPLIED_IN, RelationType.MODELS, RelationType.COUNTEREXAMPLE_TO,
           RelationType.PREREQUISITE_FOR, RelationType.DEPENDS_ON_LOGICALLY)
MAX_CONNECT = 25
SENTENCE_END = re.compile(r"[.!?…»)]$")


@dataclass(frozen=True)
class Sentence:
    section: FichaSection
    ordinal: int
    text: str
    source_id: str
    locator: str
    quote: str
    retrieved_at: str | None = None


@dataclass(frozen=True)
class Draft:
    concept: str
    lang: str
    author: str
    sentences: tuple[Sentence, ...]


# ------------------------------------------------------------------ markdown in

def parse_markdown(text: str) -> Draft:
    """Read the `ficha/1` editorial markdown (see docs in tests/test_fichas.py)."""
    head, body = _front_matter(text)
    if head.get("version") != FORMAT:
        raise ValidationError(f"ficha: version must be {FORMAT}, got {head.get('version')!r}")
    if head.get("autor") not in AUTHORS:
        raise ValidationError(f"ficha: autor must be one of {sorted(AUTHORS)}")
    section: FichaSection | None = None
    items: list[dict[str, str]] = []
    for n, raw in enumerate(body.splitlines(), 1):
        line = raw.rstrip()
        if line.startswith("## "):
            key = line[3:].strip().lower()
            if key not in HEADINGS:
                raise ValidationError(f"ficha line {n}: unknown section {line[3:]!r}")
            section = HEADINGS[key]
        elif line.startswith("- "):
            if section is None:
                raise ValidationError(f"ficha line {n}: sentence outside a section")
            items.append({"section": section.value, "text": line[2:].strip(), "line": str(n)})
        elif line.startswith("  ") and line.strip() and items:
            key, sep, value = line.strip().partition(":")
            if not sep or key.strip() not in KEYS:
                raise ValidationError(f"ficha line {n}: expected one of {sorted(KEYS)}")
            items[-1][KEYS[key.strip()]] = value.strip()
        elif line.strip() and not line.startswith("#") and not line.startswith(">"):
            raise ValidationError(f"ficha line {n}: text outside a sentence item")
    return Draft(head["concepto"], head.get("idioma", "es"), head["autor"], _sentences(items))


def _front_matter(text: str) -> tuple[dict[str, str], str]:
    parts = text.lstrip("﻿").split("---\n", 2)
    if len(parts) != 3 or parts[0].strip():
        raise ValidationError("ficha: missing --- front matter ---")
    head = {k.strip(): v.strip() for k, _, v in (l.partition(":") for l in parts[1].splitlines() if ":" in l)}
    if not head.get("concepto"):
        raise ValidationError("ficha: front matter needs `concepto`")
    return head, parts[2]


def _sentences(items: list[dict[str, str]]) -> tuple[Sentence, ...]:
    out: list[Sentence] = []
    count: dict[str, int] = {}
    for it in items:
        missing = [k for k in ("source_id", "locator", "quote") if not it.get(k)]
        if missing:
            raise ValidationError(f"ficha line {it['line']}: sentence without {', '.join(missing)}")
        if not SENTENCE_END.search(it["text"]):
            raise ValidationError(f"ficha line {it['line']}: a sentence item must end like a sentence")
        ordinal = count.get(it["section"], 0)
        count[it["section"]] = ordinal + 1
        out.append(Sentence(FichaSection(it["section"]), ordinal, it["text"], it["source_id"], it["locator"],
                            it["quote"].strip("«»\" "), it.get("retrieved_at")))
    return tuple(out)


# ------------------------------------------------------------------ registry

def note_id(concept: str, s: Sentence, lang: str) -> str:
    """Content-addressed: another text is another note (notes are immutable)."""
    digest = hashlib.sha1("\x1f".join((concept, s.section.value, str(s.ordinal), lang, s.text)).encode()).hexdigest()
    return f"note:{s.section.value}:{digest[:16]}"


def section_of(note: str) -> FichaSection | None:
    """Section encoded in a note id (`note:<section>:<hash>`), for readers that only see ids."""
    parts = note.split(":")
    return FichaSection(parts[1]) if len(parts) == 3 and parts[0] == "note" and parts[1] in FichaSection else None


def _norm(text: str) -> str:
    """Extract text as a reader sees it: without MediaWiki's `{\\displaystyle …}` copies of each formula
    (as web/src/panel.ts `cleanExtract`), invisible marks or doubled spaces."""
    out, i = [], 0
    while i < len(text):
        if text.startswith("{\\displaystyle", i):
            depth = 0
            while i < len(text):
                depth += {"{": 1, "}": -1}.get(text[i], 0)
                i += 1
                if depth == 0:
                    break
            continue
        out.append(text[i])
        i += 1
    plain = re.sub(r"[​‎‏﻿]", "", unicodedata.normalize("NFC", "".join(out)))
    return re.sub(r"\s+", " ", re.sub(r"\s+([,.;:)])", r"\1", plain)).strip()


def _wikipedia_fragment(store: Store, locator: str) -> Entity | None:
    url = urlparse(locator.split()[0])
    lang = url.netloc.split(".")[0]
    title = parse_qs(url.query).get("title", [""])[0]
    return store.get_entity(wp.fragment_id(lang, unquote(title).replace("_", " "))) if title else None


def provenance_for(store: Store, s: Sentence, author: str) -> Provenance:
    """The sentence's provenance, checked against the registry: the source exists, and for Wikipedia the
    locator is the stored revision and the quote is literally in its stored extract."""
    src = store.get_source(s.source_id)
    if src is None:
        raise ValidationError(f"unknown source {s.source_id}")
    retrieved = s.retrieved_at
    if s.source_id.startswith("src:wikipedia-"):
        frag = _wikipedia_fragment(store, s.locator)
        if frag is None or not frag.provenance:
            raise ValidationError(f"no stored Wikipedia extract for {s.locator}")
        if frag.provenance[0].locator != s.locator.split()[0].split("#")[0]:
            raise ValidationError(f"{s.locator}: the stored extract is {frag.provenance[0].locator}")
        if _norm(s.quote) not in _norm(" ".join(d.text for d in frag.descriptions)):
            raise ValidationError(f"quote not found in the stored extract of {s.locator}: {s.quote[:60]!r}")
        retrieved = frag.provenance[0].retrieved_at
    if not retrieved:
        raise ValidationError(f"{s.locator}: `recuperado` is required for sources outside the registry text")
    extractor, _, version = author.partition("/")
    return Provenance(s.source_id, s.locator, extractor, version, src.license, retrieved, s.quote)


def current_sentences(store: Store, concept: str) -> list[tuple[Claim, Entity]]:
    """Current explains claims of `concept` with their notes, in section and ordinal order."""
    out = []
    for c in store.claims_for(concept, roles=[Role.OBJECT], relations=[RelationType.EXPLAINS]):
        note = store.get_entity(c.subject)
        if note is not None and note.note is not None:
            out.append((c, note))
    order = list(FichaSection)
    return sorted(out, key=lambda cn: (order.index(cn[1].note.section), cn[1].note.ordinal))  # type: ignore[union-attr]


def import_ficha(store: Store, draft: Draft, *, actor: str, dry_run: bool = False) -> dict[str, int]:
    """Write the draft: new sentences are added, edited ones supersede the old claim at the same
    place, removed ones are retracted.  Re-importing the same markdown changes nothing."""
    if store.get_entity(draft.concept) is None:
        raise ValidationError(f"unknown concept {draft.concept}")
    planned = [(s, note_id(draft.concept, s, draft.lang), provenance_for(store, s, draft.author)) for s in draft.sentences]
    now = {(n.note.section, n.note.ordinal): (c, n) for c, n in current_sentences(store, draft.concept)}  # type: ignore[union-attr]
    counts = {"added": 0, "superseded": 0, "retracted": 0, "unchanged": 0}
    for s, nid, prov in planned:
        old = now.pop((s.section, s.ordinal), None)
        if old is not None and old[1].id == nid:
            counts["unchanged"] += 1
            continue
        counts["superseded" if old else "added"] += 1
        if dry_run:
            continue
        note = Entity(nid, EntityType.NOTE, (LangString(s.text, draft.lang),), statements=(LangString(s.text, draft.lang),),
                      provenance=(prov,), note=NoteInfo(s.section, s.ordinal))
        store.add_entity(note, actor=actor, on_conflict="ignore")
        prior = len(store.claims_for(nid, current_only=False))
        claim = binary(stable_id("c", "explains", nid, draft.concept, str(prior)), RelationType.EXPLAINS, nid,
                       draft.concept, (prov,), evidence_state=EvidenceState.MODEL_PROPOSED)
        if old:
            store.supersede(old[0].id, claim, actor=actor, reason="ficha import: sentence edited")
        else:
            store.add_claim(claim, actor=actor)
    for c, _ in now.values():
        counts["retracted"] += 1
        if not dry_run:
            store.set_evidence_state(c.id, EvidenceState.RETRACTED, actor=actor, reason="ficha import: sentence removed")
    return counts


def review(store: Store, claim_ids: list[str], *, reviewer: str, reason: str) -> int:
    """A person marks sentences as reviewed.  Never called by an importer or a model."""
    if not reviewer.strip() or reviewer.startswith(("draft", "system", "curation", "atlas")):
        raise ValidationError("review needs a person as reviewer")
    for cid in claim_ids:
        c = store.get_claim(cid)
        if c is None or c.relation is not RelationType.EXPLAINS:
            raise ValidationError(f"{cid} is not an explains claim")
        store.set_evidence_state(cid, EvidenceState.EDITORIALLY_REVIEWED, actor=reviewer, reason=reason)
    return len(claim_ids)


# ------------------------------------------------------------------ markdown out

def render_ficha(store: Store, concept: str, *, include_drafts: bool = False) -> str:
    entity = store.get_entity(concept)
    if entity is None:
        raise ValidationError(f"unknown concept {concept}")
    notes: list[str] = []

    def cite(text: str) -> str:
        notes.append(text)
        return f"[^{len(notes)}]"

    sentences = current_sentences(store, concept)
    quoted: set[str] = set()  # licences of the source text shown verbatim
    reviewed = [cn for cn in sentences if cn[0].evidence_state is EvidenceState.EDITORIALLY_REVIEWED]
    body: list[str] = []
    for section in FichaSection:
        body += ["", f"## {TITLES[section]}"]
        mine = [cn for cn in sentences if cn[1].note.section is section]  # type: ignore[union-attr]
        done = [_sentence(c, n, cite) for c, n in mine if c.evidence_state is EvidenceState.EDITORIALLY_REVIEWED]
        body += done or ["_Todavía no hay una explicación revisada._"]
        drafts = [_sentence(c, n, cite) for c, n in mine if c.evidence_state is not EvidenceState.EDITORIALLY_REVIEWED]
        if include_drafts and drafts:
            body += ["", f"### {DRAFT_LABEL}", *(f"- {d}" for d in drafts)]
        if section is FichaSection.QUE_ES:
            body += _extract(store, concept, cite, quoted)
        if section is FichaSection.POR_QUE_IMPORTA:
            body += ["", "## Qué saber antes", *_claim_lines(store, concept, BEFORE, cite)]
            body += ["", "## Con qué se conecta", *_claim_lines(store, concept, None, cite)]
    licences = sorted(quoted | {p.license for c, _ in (reviewed if not include_drafts else sentences) for p in c.provenance})
    head = ["---", f"concepto: {concept}", f"titulo: {entity.label('es')}", f"version: {FORMAT}",
            f"generada: {now_iso()} desde el registro (event_seq {_last_seq(store)})",
            f"estado: {len(reviewed)} de {len(sentences)} frases revisadas",
            f"licencia: {_licence(licences)}", "---"]
    return "\n".join(head + body + [""] + [f"[^{i}]: {t}" for i, t in enumerate(notes, 1)]) + "\n"


def _sentence(c: Claim, n: Entity, cite: Callable[[str], str]) -> str:
    p = c.provenance[0]
    return f"{n.statements[0].text if n.statements else n.labels[0].text} " + cite(
        f"{p.source_id}, {p.locator} · {p.extractor}/{p.extractor_version} · {c.evidence_state.value} · "
        f"{p.retrieved_at} · {c.id} · cita: «{p.quote}»")


def _extract(store: Store, concept: str, cite: Callable[[str], str], licences: set[str]) -> list[str]:
    for lang in ("es", "en"):
        x = wp.lead_extract(store, concept, lang)
        if x:
            licences.add(x["license"])
            return ["", f"### Extracto de Wikipedia ({lang}), sin revisar", "", f"> {_norm(x['text'])} " + cite(x["attribution"])]
    return []


def _claim_lines(store: Store, concept: str, only: dict | None, cite: Callable[[str], str]) -> list[str]:
    rels = list(only) if only else list(CONNECT)
    lines: list[str] = []
    for c in store.claims_for(concept, relations=rels):
        role = Role.SUBJECT if c.subject == concept else Role.OBJECT
        if only is not None and only[c.relation] is not role:
            continue
        if only is None and c.relation in BEFORE and BEFORE[c.relation] is role:
            continue  # already under «Qué saber antes»
        others = list(c.targets) if role is Role.SUBJECT else [c.subject]
        name = lambda i: (e.label("es") if (e := store.get_entity(i)) else i)  # noqa: E731
        p = c.provenance[0]
        verb = c.relation.value if role is Role.SUBJECT else f"{c.relation.value} (inversa)"
        if c.is_conjunction:  # the conjunction is kept whole (principle 3): every premise, never only the first
            rest = [t for t in c.targets if t != concept]
            joint = (f" (conjunción de {len(c.targets)} premisas juntas)" if role is Role.SUBJECT
                     else f" (junto con {', '.join(name(t) for t in rest)})" if rest else "")
        else:
            joint = ""
        lines.append(f"- {' y '.join(name(o) for o in others)}{joint} — `{verb}`, {c.evidence_state.value}, {c.id} "
                     + cite(f"{p.source_id}, {p.locator} · {p.extractor}/{p.extractor_version}"))
    if not lines:
        return ["_No hay conexiones registradas de este tipo._"]
    extra = len(lines) - MAX_CONNECT
    return lines[:MAX_CONNECT] + ([f"- … y {extra} más en el registro."] if extra > 0 else [])


def _licence(licences: list[str]) -> str:
    if any("BY-SA" in l for l in licences):
        return "CC BY-SA 4.0, con atribución a las fuentes citadas en cada frase"
    return ", ".join(licences) or "sin texto de fuentes"


def _last_seq(store: Store) -> int:
    # ponytail: private connection; a Store.last_event_seq() in the contract removes it.
    return store._conn.execute("SELECT COALESCE(MAX(seq), 0) FROM events").fetchone()[0]


# ------------------------------------------------------------------ CLI

def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="python -m atlas fichas", description=__doc__.split("\n\n")[0])
    ap.add_argument("--db", default="data/atlas.db")
    sub = ap.add_subparsers(dest="cmd", required=True)
    i = sub.add_parser("import", help="editorial markdown -> notes + explains (model_proposed)")
    i.add_argument("files", nargs="+")
    i.add_argument("--actor", default="atlas.fichas/1")
    i.add_argument("--dry-run", action="store_true", help="parse and check sources and quotes; write nothing")
    r = sub.add_parser("render", help="registry -> markdown view")
    r.add_argument("ids", nargs="+")
    r.add_argument("--drafts", action="store_true", help="also show unreviewed sentences, labelled as such")
    r.add_argument("--out", default="data/fichas")
    v = sub.add_parser("review", help="a person marks sentences editorially_reviewed")
    v.add_argument("claims", nargs="+")
    v.add_argument("--reviewer", required=True)
    v.add_argument("--reason", default="revisión editorial de la ficha")
    a = ap.parse_args(argv)
    sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
    try:
        with Store(a.db, read_only=a.cmd == "render" or (a.cmd == "import" and a.dry_run)) as st:
            if a.cmd == "import":
                for f in a.files:
                    print(f, import_ficha(st, parse_markdown(Path(f).read_text(encoding="utf-8")), actor=a.actor,
                                          dry_run=a.dry_run))
            elif a.cmd == "render":
                Path(a.out).mkdir(parents=True, exist_ok=True)
                for cid in a.ids:
                    out = Path(a.out) / (re.sub(r"[^A-Za-z0-9_.-]", "_", cid) + ".md")
                    out.write_text(render_ficha(st, cid, include_drafts=a.drafts), encoding="utf-8")
                    print(out)
            else:
                print(review(st, a.claims, reviewer=a.reviewer, reason=a.reason), "reviewed")
    except (ValidationError, OSError) as exc:
        print(f"atlas fichas: {exc}", file=sys.stderr)
        return 1
    return 0
