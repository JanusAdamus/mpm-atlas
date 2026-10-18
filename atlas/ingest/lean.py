"""Source-level parser for Lean 4 / Mathlib files (no elaboration).

`parse_file(path, text)` is a pure function (picklable result) so the Mathlib
adapter can run it in a process pool.  It recovers what can be read from the
text alone: imports, the module doc title, and named declarations with kind,
fully qualified name (namespaces tracked), file line, signature up to the
top-level `:=` / `where` / `|`, the doc comment, `@[wikidata Q…]` attributes and
`alias` targets.  What needs the elaborator (auto-generated names such as
`to_additive` duplicates, anonymous instance names, axioms used) is not invented:
it is counted as skipped.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

PARSER_VERSION = "lean-src/1"
MAX_SIGNATURE = 2000
MAX_DOC = 4000

KIND_STATEMENT = frozenset({"theorem", "lemma", "axiom"})
DECL_KINDS = ("theorem", "lemma", "def", "abbrev", "structure", "class inductive", "class", "inductive",
              "instance", "opaque", "axiom", "alias", "irreducible_def")

_ATTR = r"@\[(?:[^\[\]]|\[[^\[\]]*\])*\]"
_MODS = r"(?:(?:private|protected|noncomputable|nonrec|partial|unsafe|public|scoped|local|meta)\s+)*"
DECL_RE = re.compile(
    rf"^[ \t]*(?:open\b[^\n]*?\bin\s+)?(?P<attrs>(?:{_ATTR}\s*)*)(?P<mods>{_MODS})"
    rf"(?P<kw>theorem|lemma|def|abbrev|structure|class\s+inductive|class|inductive|instance|opaque|axiom|alias"
    rf"|irreducible_def)\b",
    re.M)
SCOPE_RE = re.compile(
    rf"^[ \t]*(?:(?P<ns>namespace)\s+(?P<nsname>\S+)"
    rf"|(?:{_ATTR}\s*)?(?:(?:public|private|noncomputable|meta)\s+)*(?P<sec>section)\b[ \t]*(?P<secname>[^\s-]*)"
    rf"|(?P<mutual>mutual)\b"
    rf"|(?P<end>end)\b[ \t]*(?P<endname>[^\s-]*))",
    re.M)
IMPORT_RE = re.compile(r"^[ \t]*(?:(?:public|private|meta)\s+)*import\s+(?:all\s+)?(?P<mods>[^\n]+)", re.M)
OPEN_RE = re.compile(r"^[ \t]*open\s+(?:scoped\s+)?(?P<names>[^\n]+)", re.M)
NAME_RE = re.compile(r"«[^»]+»(?:\.[^\s:({\[⦃⟨]+)?|[^\s:({\[⦃⟨]+")
WIKIDATA_RE = re.compile(r"\bwikidata\s+(Q\d+)")
PRIORITY_RE = re.compile(r"^\s*\(priority\s*:=\s*[^)]*\)\s*")
OPENERS, CLOSERS = "([{⟨⦃", ")]}⟩⦄"


@dataclass(frozen=True)
class Decl:
    name: str  # fully qualified
    kind: str  # theorem | lemma | def | ... | alias
    line: int
    signature: str
    doc: str | None
    wikidata: tuple[str, ...] = ()
    alias_of: str | None = None  # for `alias name := target`: candidate full names, best first
    alias_candidates: tuple[str, ...] = ()
    deprecated: bool = False
    namespace: tuple[str, ...] = ()


@dataclass(frozen=True)
class FileResult:
    path: str
    module: str
    imports: tuple[str, ...]
    title: str | None  # first `# heading` of the module doc
    decls: tuple[Decl, ...]
    skipped: dict[str, int] = field(default_factory=dict)
    error: str | None = None
    opens: tuple[str, ...] = ()  # every namespace `open`ed anywhere in the file (scoping ignored)


def module_name(path: str) -> str:
    """'Mathlib/Data/Nat/Prime/Defs.lean' -> 'Mathlib.Data.Nat.Prime.Defs'."""
    p = path.replace("\\", "/")
    return p[: -len(".lean")].replace("/", ".") if p.endswith(".lean") else p.replace("/", ".")


def parse_file(path: str, text: str) -> FileResult:
    try:
        return _parse(path, text)
    except Exception as exc:  # recorded per file; one bad file never stops the run
        return FileResult(path, module_name(path), (), None, (), {}, f"{type(exc).__name__}: {exc}")


# ---------------------------------------------------------------------- comment stripping

_TOKEN_RE = re.compile(r'--|/-|"')


def strip_comments(text: str) -> tuple[str, list[tuple[int, int, str, str]]]:
    """Replace comments and string contents with spaces (newlines kept, so offsets and
    line numbers survive).  Returns (code, docs) with docs = [(start, end, kind, body)],
    kind '/--' (declaration doc) or '/-!' (module doc)."""
    out = list(text)
    docs: list[tuple[int, int, str, str]] = []
    i, n = 0, len(text)
    while i < n:
        m = _TOKEN_RE.search(text, i)
        if m is None:
            break
        s = m.start()
        tok = m.group()
        if tok == "--":
            e = text.find("\n", s)
            e = n if e < 0 else e
        elif tok == '"':
            e = s + 1
            while e < n and text[e] != '"':
                e += 2 if text[e] == "\\" else 1
            e = min(e + 1, n)
            _blank(out, s + 1, e - 1)
            i = e
            continue
        else:  # block comment, nested
            depth, e = 1, s + 2
            while e < n and depth:
                if text.startswith("/-", e):
                    depth += 1
                    e += 2
                elif text.startswith("-/", e):
                    depth -= 1
                    e += 2
                else:
                    e += 1
            kind = text[s : s + 3]
            if kind in ("/--", "/-!"):
                docs.append((s, e, kind, text[s + 3 : max(s + 3, e - 2)].strip()))
        _blank(out, s, e)
        i = e
    return "".join(out), docs


def _blank(buf: list[str], s: int, e: int) -> None:
    for k in range(s, e):
        if buf[k] != "\n":
            buf[k] = " "


# ---------------------------------------------------------------------- main parse


def _parse(path: str, text: str) -> FileResult:
    code, docs = strip_comments(text)
    module = module_name(path)
    imports = tuple(dict.fromkeys(
        tok for m in IMPORT_RE.finditer(code) for tok in m.group("mods").split() if tok[0].isalpha()))
    title = _module_title(docs)
    doc_by_end = {e: body for s, e, kind, body in docs if kind == "/--"}
    line_starts = _line_starts(text)
    events = sorted([(m.start(), "decl", m) for m in DECL_RE.finditer(code)]
                    + [(m.start(), "scope", m) for m in SCOPE_RE.finditer(code)], key=lambda x: x[0])
    decl_starts = [pos for pos, kind, _ in events if kind == "decl"] + [len(code)]
    stack: list[tuple[str, list[str]]] = []
    decls: list[Decl] = []
    skipped: dict[str, int] = {}
    di = 0
    for pos, kind, m in events:
        if kind == "scope":
            _apply_scope(stack, m)
            continue
        di += 1
        nxt = next((p for p in decl_starts[di - 1 :] if p > pos), len(code))
        d = _decl(m, code, text, pos, nxt, stack, doc_by_end, line_starts, skipped)
        if d is not None:
            decls.append(d)
    opens = tuple(dict.fromkeys(
        tok for m in OPEN_RE.finditer(code) for tok in m.group("names").split("(")[0].split()
        if tok not in ("in", "hiding", "renaming", "scoped") and tok[:1].isupper()))
    return FileResult(path, module, imports, title, tuple(decls), skipped, None, opens)


def _apply_scope(stack: list[tuple[str, list[str]]], m: re.Match) -> None:
    if m.group("ns"):
        stack.append(("namespace", m.group("nsname").split(".")))
    elif m.group("sec"):
        stack.append(("section", []))
    elif m.group("mutual"):
        stack.append(("mutual", []))
    elif m.group("end") and stack:
        stack.pop()


def _decl(m: re.Match, code: str, text: str, pos: int, nxt: int, stack, doc_by_end, line_starts,
          skipped: dict[str, int]) -> Decl | None:
    kw = " ".join(m.group("kw").split())
    mods = m.group("mods").split()
    attrs = m.group("attrs")
    if "private" in mods:
        skipped["private"] = skipped.get("private", 0) + 1
        return None
    rest_start = m.end()
    body = code[rest_start:nxt]
    if kw == "instance":
        body = PRIORITY_RE.sub("", body, count=1)
    head = body.lstrip()
    if kw == "alias" and head.startswith("⟨"):
        skipped["alias_iff_pair"] = skipped.get("alias_iff_pair", 0) + 1
        return None
    nm = NAME_RE.match(head)
    if nm is None or nm.group() in (":=", "where") or (kw == "instance" and _looks_anonymous(head)):
        key = "anonymous_instance" if kw == "instance" else f"unnamed_{kw}"
        skipped[key] = skipped.get(key, 0) + 1
        return None
    short = nm.group().strip()
    after = head[nm.end():]
    if short.endswith(".") and after.startswith("{"):  # explicit universes: `def Foo.{u} ...`
        short = short[:-1]
        after = after[after.find("}") + 1:]
    if not short or short.endswith("."):
        skipped["unparsed_name"] = skipped.get("unparsed_name", 0) + 1
        return None
    ns = [p for scope, parts in stack if scope == "namespace" for p in parts]
    if short.startswith("_root_."):
        full = short[len("_root_."):]
    else:
        full = ".".join(ns + [short]) if ns else short
    signature = _signature(after)
    doc = _doc_before(text, pos, doc_by_end)
    alias_cands: tuple[str, ...] = ()
    if kw == "alias":
        target = after.split(":=", 1)[1].split()[0] if ":=" in after else ""
        if not target:
            skipped["alias_unparsed"] = skipped.get("alias_unparsed", 0) + 1
            return None
        alias_cands = _resolution_candidates(target, ns)
    line = _line_of(line_starts, m.start("kw"))
    wd = tuple(dict.fromkeys(WIKIDATA_RE.findall(attrs)))
    return Decl(full, kw, line, signature, doc, wd, alias_cands[0] if alias_cands else None, alias_cands,
                "deprecated" in attrs, tuple(ns))


def _looks_anonymous(head: str) -> bool:
    return head[:1] in (":", "[", "(", "{", "⦃") or head.startswith("where")


def _signature(after: str) -> str:
    """Text up to the first top-level `:=`, `where`, `|` (equation compiler) or `deriving`."""
    depth, i, n = 0, 0, len(after)
    limit = min(n, 20000)
    while i < limit:
        ch = after[i]
        if ch in OPENERS:
            depth += 1
        elif ch in CLOSERS:
            depth = max(0, depth - 1)
        elif depth == 0:
            if after.startswith(":=", i):
                break
            if ch == "|" and _line_begins(after, i):
                break
            if ch in "wd" and (i == 0 or not (after[i - 1].isalnum() or after[i - 1] in "_.'")):
                if re.match(r"(where|deriving)\b", after[i:i + 9]):
                    break
        i += 1
    sig = " ".join(after[:i].split())
    return sig[:MAX_SIGNATURE]


def _line_begins(s: str, i: int) -> bool:
    j = i - 1
    while j >= 0 and s[j] in " \t":
        j -= 1
    return j < 0 or s[j] == "\n"


def _doc_before(text: str, pos: int, doc_by_end: dict[int, str]) -> str | None:
    j = pos
    while j > 0 and text[j - 1] in " \t\r\n":
        j -= 1
    body = doc_by_end.get(j)
    if body is None:
        return None
    body = " ".join(body.split())
    return body[:MAX_DOC] or None


def _resolution_candidates(target: str, ns: list[str]) -> tuple[str, ...]:
    """Lean resolves a relative name against enclosing namespaces, innermost first."""
    if target.startswith("_root_."):
        return (target[len("_root_."):],)
    cands = [".".join(ns[:k] + [target]) for k in range(len(ns), 0, -1)] + [target]
    return tuple(dict.fromkeys(cands))


def _module_title(docs: list[tuple[int, int, str, str]]) -> str | None:
    for _, _, kind, body in docs:
        if kind != "/-!":
            continue
        for line in body.splitlines():
            line = line.strip()
            if line.startswith("#"):
                return line.lstrip("#").strip() or None
        return None
    return None


def _line_starts(text: str) -> list[int]:
    starts = [0]
    for m in re.finditer("\n", text):
        starts.append(m.end())
    return starts


def _line_of(starts: list[int], pos: int) -> int:
    lo, hi = 0, len(starts) - 1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if starts[mid] <= pos:
            lo = mid
        else:
            hi = mid - 1
    return lo + 1


def name_tokens(name: str) -> list[str]:
    """'Nat.Prime.two_le' -> ['nat', 'prime', 'two', 'le']; splits CamelCase and snake_case."""
    out: list[str] = []
    for part in re.split(r"[._'\s]+", name.replace("«", "").replace("»", "")):
        for t in re.findall(r"[A-Z]+(?=[A-Z][a-z])|[A-Z]?[a-z]+|[A-Z]+|\d+|[^\W\d_]+", part):
            out.append(t.lower())
    return out
