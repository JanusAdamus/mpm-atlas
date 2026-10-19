// Record of one entity (round 4): five questions always in the same order (what it is, why
// it matters, what to know first, what it connects to, how to check), each with its source
// or an honest empty state.  No mathematical text of our own: whatever does not come from a
// source is marked as an unreviewed editorial draft.  The explore mode hides ids, code
// records, provenance and scores; the technical mode shows them as before.
import type { ClaimJson, EntityJson, EntityResponse } from "./api.ts";
import { h, safeLink } from "./dom.ts";
import { badge, sourceTitle } from "./evidence.ts";
import { getLang, isExplore, plainVerb, relLabel, sealName, stateLabel, t, typeLabel } from "./i18n.ts";
import { readerLabel } from "./pathview.ts";
import { typeColor } from "./theme.ts";
import { isCodeId, jargonIn, relationInfo } from "./vocab.ts";

export interface PanelActions {
  open(id: string): void;
  showOnMap(id: string): void;
  focusRegion(id: string): void;
  pathFrom(id: string, label: string): void;
  pathTo(id: string, label: string): void;
  openConcept(id: string): void;
  /** Fetch the next page of claims (shown when the answer is truncated). */
  loadMore?(): void;
  /** Fetch a Wikipedia fragment (`wp:` id) for the «Qué es» extract. */
  extract?(id: string): Promise<EntityResponse>;
  /** A page of the entity's claims from `offset` (as many as the server allows), to find its
   *  Wikipedia article when the first page did not reach it (C4-04). */
  claimsFrom?(offset: number): Promise<EntityResponse>;
}

/** The entity answer with the next page of claims appended (`next` came with `offset` = claims shown). */
export function mergeEntityPage(prev: EntityResponse, next: EntityResponse): EntityResponse {
  const seen = new Set(prev.claims.map((c) => c.id));
  return {
    ...prev,
    truncated: next.truncated,
    claims: [...prev.claims, ...next.claims.filter((c) => !seen.has(c.id))],
    sources: { ...prev.sources, ...next.sources },
  };
}

/** The entity's short description comes from Wikidata: labelled as such, never as an editorial explanation. */
export const fromWikidata = (e: Pick<EntityJson, "provenance">): boolean => e.provenance.some((p) => p.source_id === "src:wikidata");

/** Extraction limits recorded as `assumptions` by the importers (atlas/ingest/mathlib.py):
 *  they describe the verification status, never a hypothesis of the mathematics. */
const LIMITS: [RegExp, string][] = [
  [/^axioms not computed/i, "limit_axioms"],
  [/resolved by name from source text/i, "limit_sigdep"],
];

export function extractionLimit(a: string): string | null {
  const hit = LIMITS.find(([re]) => re.test(a));
  return hit ? hit[1] : null;
}

/** Model-proposed and disputed claims are candidates: never under the heading of established ones. */
export const isCandidate = (c: Pick<ClaimJson, "evidence_state">): boolean =>
  c.evidence_state === "model_proposed" || c.evidence_state === "disputed";

/** Reviewed by an editor or linked to a formal proof: the only claims not attributed to their source. */
const isReviewed = (c: Pick<ClaimJson, "evidence_state">): boolean =>
  c.evidence_state === "editorially_reviewed" || c.evidence_state === "formally_linked";

/** Text in the UI language, else the other one, tagged with its language. */
function localized(m: Record<string, string>): { text: string; lang: string } | null {
  const l = getLang();
  if (m[l]) return { text: m[l], lang: l };
  const other = Object.entries(m)[0];
  return other ? { text: other[1], lang: other[0] } : null;
}

let math: Promise<typeof import("./math.ts")> | null = null;

/** Typeset `tex` into `el` once KaTeX has loaded; until then (or on error) the source shows. */
function typeset(el: HTMLElement, tex: string, display: boolean): void {
  el.textContent = tex;
  math ??= import("./math.ts");
  math.then((m) => m.renderMath(el, tex, display)).catch(() => {
    el.textContent = tex;
  });
}

const MATHML_NS = "http://www.w3.org/1998/Math/MathML";
const MATHML_TAGS = new Set([
  "math", "mrow", "mi", "mn", "mo", "ms", "mtext", "mspace", "msup", "msub", "msubsup", "mfrac", "msqrt", "mroot",
  "mstyle", "mover", "munder", "munderover", "mtable", "mtr", "mtd", "mpadded", "mphantom", "menclose", "mfenced", "semantics",
]);
const MATHML_ATTRS = new Set([
  "mathvariant", "displaystyle", "scriptlevel", "stretchy", "fence", "separator", "lspace", "rspace", "accent",
  "accentunder", "columnalign", "rowspacing", "columnspacing", "linethickness", "form", "largeop", "movablelimits",
  "symmetric", "minsize", "maxsize", "notation", "open", "close", "separators", "width", "height", "depth", "display",
]);

/** Some registry formulas arrive as MathML (Wikidata P2534 rendered by MediaWiki), not TeX.
 *  Rebuild them element by element from an allow-list (no attributes that can carry code,
 *  no foreign elements), and use an embedded TeX annotation through KaTeX when present.
 *  Returns null when the string is not usable MathML. */
function mathmlNode(src: string): { node?: Element; tex?: string } | null {
  const doc = new DOMParser().parseFromString(src, "application/xml");
  const root = doc.documentElement;
  if (root.localName !== "math" || doc.getElementsByTagName("parsererror").length) return null;
  const tex = [...root.getElementsByTagName("annotation")].find((a) => a.getAttribute("encoding") === "application/x-tex")?.textContent?.trim();
  if (tex) return { tex };
  const copy = (n: Element): Element | null => {
    if (!MATHML_TAGS.has(n.localName)) return null;
    const out = document.createElementNS(MATHML_NS, n.localName);
    for (const a of [...n.attributes]) if (MATHML_ATTRS.has(a.name)) out.setAttribute(a.name, a.value);
    for (const c of [...n.childNodes]) {
      if (c.nodeType === Node.TEXT_NODE) out.append(c.textContent ?? "");
      else if (c.nodeType === Node.ELEMENT_NODE) {
        const k = copy(c as Element);
        if (k) out.append(k);
      }
    }
    return out;
  };
  const node = copy(root);
  if (node) node.setAttribute("display", "block");
  return node ? { node } : null;
}

export function renderFormula(src: string): HTMLElement {
  const isMathml = /^\s*<math[\s>]/.test(src);
  const parsed = isMathml ? mathmlNode(src) : null;
  const tex = parsed?.tex ?? (isMathml ? null : src);
  const el = h("div", { class: "formula" });
  if (tex !== null) {
    el.setAttribute("role", "math");
    el.setAttribute("aria-label", tex);
    typeset(el, tex, true);
  } else if (parsed?.node) {
    el.append(parsed.node); // native MathML carries its own semantics
  } else {
    el.textContent = src.replace(/<[^>]*>/g, " ").replace(/\s+/g, " ").trim();
  }
  return el;
}

/** Inline math in prose: `$…$` segments typeset with KaTeX, the rest as text. */
export function richText(s: string): HTMLElement {
  const el = h("span");
  for (const p of s.split(/(\$[^$\n]{1,400}\$)/g)) {
    if (p.length > 2 && p.startsWith("$") && p.endsWith("$")) {
      const span = h("span", { class: "math-inline" });
      typeset(span, p.slice(1, -1), false);
      el.append(span);
    } else if (p) el.append(p);
  }
  return el;
}

function entityLink(id: string, label: string | undefined, actions: PanelActions): HTMLElement {
  // In the explore mode a missing label never falls back to the raw id.
  const text = isExplore() ? readerLabel(label ?? "") : label || id;
  return h("button", { class: "link", type: "button", title: isExplore() ? text : id, onclick: () => actions.open(id) }, text);
}

/** The role the entity plays in a claim (it may appear under a merged id). */
const roleOf = (c: ClaimJson, selfIds: Set<string>): string => c.participants.find((p) => selfIds.has(p.entity))?.role ?? "subject";

/** The other end(s) of a claim, seen from the entity. */
function othersOf(c: ClaimJson, selfIds: Set<string>): { entity: string; role: string }[] {
  const role = roleOf(c, selfIds);
  if (role === "subject") return c.participants.filter((p) => (p.role === "object" || p.role === "premise") && !selfIds.has(p.entity));
  return c.participants.filter((p) => p.role === "subject" && !selfIds.has(p.entity));
}

/** A claim the explore mode leaves to the technical one: code records and concept groupings. */
export const isTechnicalClaim = (c: ClaimJson): boolean =>
  c.relation === "member_of_concept" || c.participants.some((p) => isCodeId(p.entity) || jargonIn(c.labels[p.entity] ?? "").length > 0);

/** The relation read from the entity's side, an inverse pair folded into one name: «A
 *  generaliza B» seen from B is «B es un caso particular de A». */
function senseOf(c: ClaimJson, selfIds: Set<string>): { rel: string; forward: boolean } {
  const forward = roleOf(c, selfIds) === "subject";
  const inv = relationInfo(c.relation)?.inverse;
  return inv && inv < c.relation ? { rel: inv, forward: !forward } : { rel: c.relation, forward };
}

/** Claims that record one relation with the same other end in both directions (Wikidata
 *  says the theorem generalizes the postulate and the postulate generalizes the theorem):
 *  a conflict to review, never two facts.  Symmetric relations cannot conflict this way. */
export function conflictingClaims(claims: ClaimJson[], selfIds: Set<string>): Set<string> {
  const keys = (c: ClaimJson): string[] =>
    relationInfo(c.relation)?.symmetric ? [] : othersOf(c, selfIds).map((p) => `${senseOf(c, selfIds).rel}|${p.entity}`);
  const senses = new Map<string, Set<boolean>>();
  for (const c of claims) for (const k of keys(c)) senses.set(k, new Set([...(senses.get(k) ?? []), senseOf(c, selfIds).forward]));
  return new Set(claims.filter((c) => keys(c).some((k) => senses.get(k)?.size === 2)).map((c) => c.id));
}

/** Claims shown as one line: same relation, same side, same other end(s). */
export interface Row { relation: string; inverse: boolean; others: { entity: string; role: string }[]; claims: ClaimJson[] }

/** One row per relation and destination (Wikidata states «matriz forma parte de álgebra
 *  lineal» from three properties): the presentation is merged, every claim is kept. */
export function dedupRows(claims: ClaimJson[], selfIds: Set<string>): Row[] {
  const rows = new Map<string, Row>();
  for (const c of claims) {
    const others = othersOf(c, selfIds);
    const inverse = roleOf(c, selfIds) !== "subject";
    const key = `${c.relation}${inverse ? "~" : ""}|${others.map((p) => p.entity).sort().join(",")}`;
    const row = rows.get(key);
    rows.set(key, row ? { ...row, claims: [...row.claims, c] } : { relation: c.relation, inverse, others, claims: [c] });
  }
  return [...rows.values()];
}

/** Technical row: every claim with its roles, provenance and scores. */
function claimRow(c: ClaimJson, self: string, selfIds: Set<string>, actions: PanelActions): HTMLElement {
  const role = c.participants.find((p) => selfIds.has(p.entity))?.role ?? "subject";
  const others = (roles: string[]) => c.participants.filter((p) => roles.includes(p.role) && !selfIds.has(p.entity));
  const targets = others(["object", "premise"]);
  const premises = c.participants.filter((p) => p.role === "premise");
  const subject = c.participants.find((p) => p.role === "subject");
  const label = (id: string) => c.labels[id];
  const line = h("div", { class: "claim-line" });
  if (role === "subject") {
    line.append(h("span", { class: "dir", "aria-label": "→" }, "→ "));
    targets.forEach((p, i) => {
      if (i > 0) line.append(h("span", { class: "and" }, premises.length > 1 ? " + " : ", "));
      line.append(entityLink(p.entity, label(p.entity), actions));
    });
  } else if (subject) {
    line.append(h("span", { class: "dir", "aria-label": "←" }, "← "), entityLink(subject.entity, label(subject.entity), actions));
    if (role === "premise" && premises.length > 1) {
      line.append(h("span", { class: "muted" }, ` (${t("premises").toLowerCase()}: ${premises.length})`));
    } else if (role !== "object") line.append(h("span", { class: "muted" }, ` (${t(`role_${role}`)})`));
  }
  const meta = h("div", { class: "claim-meta" }, badge(c.evidence_state));
  if (premises.length > 1) meta.append(h("span", { class: "tag" }, t("jointPremises", { n: premises.length })));
  if (c.alternative_group) meta.append(h("span", { class: "tag", title: c.alternative_group }, `${t("alternatives")}: ${c.alternative_group}`));
  if (c.formal) meta.append(h("span", { class: "tag" }, `⊢ ${c.formal.library} ${c.formal.version}: ${c.formal.declaration}`));
  // A heuristic score is labelled as such, never shown as a probability (Codex E02).
  if (c.confidence) meta.append(h("span", { class: "tag", title: `${c.confidence.estimates} · ${c.confidence.calibration}` }, `${t("heuristicScore")}: ${c.confidence.value.toFixed(2)}`));
  const extra: HTMLElement[] = [];
  const via = others(["via"]), ctx = others(["context"]), assm = others(["assumption"]);
  if (via.length) extra.push(h("div", { class: "muted" }, `${t("via")}: `, ...via.map((p) => entityLink(p.entity, label(p.entity), actions))));
  if (ctx.length) extra.push(h("div", { class: "muted" }, `${t("context")}: `, ...ctx.map((p) => entityLink(p.entity, label(p.entity), actions))));
  const hyp = c.assumptions.filter((a) => !extractionLimit(a));
  const limits = c.assumptions.map(extractionLimit).filter((k): k is string => !!k);
  if (hyp.length || assm.length) {
    extra.push(h("div", { class: "assumptions" }, `${t("assumptions")}: `, hyp.join("; "), ...assm.map((p) => entityLink(p.entity, label(p.entity), actions))));
  }
  if (limits.length) extra.push(h("div", { class: "muted small" }, `${t("verification")}: `, limits.map((k) => t(k)).join(" ")));
  if (c.validity_domain) extra.push(h("div", { class: "muted" }, `⟂ ${c.validity_domain}`));
  const expl = explanationOf(c);
  if (expl) extra.push(expl);
  if (c.disagreements.length) extra.push(h("div", { class: "muted" }, `≠ ${c.disagreements.join("; ")}`));
  const prov = c.provenance.map((p) => `${p.source_id} · ${p.locator}`).join("; ");
  return h("li", { class: `claim state-${c.evidence_state}`, "data-claim": c.id, title: `${c.id} — ${self}` }, line, meta, ...extra, h("div", { class: "prov" }, prov));
}

/** A claim's explanation: reviewed only when the claim is; otherwise marked as an unreviewed draft. */
function explanationOf(c: ClaimJson): HTMLElement | null {
  const expl = localized(c.explanation);
  if (!expl) return null;
  const reviewed = c.evidence_state === "editorially_reviewed";
  // Unreviewed explanations are the extractors' rationale (how a candidate was proposed):
  // technical detail, left out of the explore mode.
  if (!reviewed && isExplore()) return null;
  return h("div", { class: "explanation", lang: expl.lang },
    reviewed ? null : h("span", { class: "draft-tag" }, `${t("editorialDraft")}: `), richText(expl.text));
}

/** Plain row: one sentence read from the entity's side; an unreviewed relation is said in its
 *  source's voice («Según Wikidata, …»), with its seal and how many source records back it.
 *  No ids, provenance or scores. */
function sentenceRow(row: Row, r: EntityResponse, actions: PanelActions, tag: HTMLElement | null = null): HTMLElement {
  const c0 = row.claims[0];
  const prov = row.claims.flatMap((c) => c.provenance);
  const sources = [...new Set(prov.map((p) => sourceName(r, p.source_id)))];
  const lead = row.claims.some(isReviewed) || !sources.length
    ? null : t("accordingTo", { src: new Intl.ListFormat(getLang(), { type: "conjunction" }).format(sources) });
  const records = new Set(prov.map((p) => `${p.source_id} ${p.locator}`)).size;
  const states = [...new Set(row.claims.map((c) => c.evidence_state))];
  const line = h("div", { class: "claim-line sentence" }, lead, `${r.label} ${plainVerb(row.relation, row.inverse)} `,
    ...row.others.flatMap((p, i) => [i > 0 ? ", " : null, entityLink(p.entity, c0.labels[p.entity], actions)]), ".");
  const meta = h("div", { class: "claim-meta" }, ...states.map(badge), records > 1 ? h("span", { class: "tag" }, t("sourceRecords", { n: records })) : null, tag);
  return h("li", { class: `claim state-${c0.evidence_state}${tag ? " conflict" : ""}`, "data-claim": c0.id }, line, meta, explanationOf(c0));
}

// ---- the five questions ----------------------------------------------------------------

/** Wikipedia text: MediaWiki leaves `{\displaystyle …}` after each formula and zero-width
 *  spaces after references; drop both and keep the first sentences (at most `max` chars). */
export function cleanExtract(text: string, max = 420): string {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    if (text.startsWith("{\\displaystyle", i)) {
      let depth = 0;
      for (; i < text.length; i++) {
        if (text[i] === "{") depth++;
        else if (text[i] === "}" && --depth === 0) break;
      }
      continue;
    }
    out += text[i];
  }
  out = out.replace(/[​‎‏]/g, "").replace(/\s+([,.;:)])/g, "$1").replace(/\s+/g, " ").trim();
  if (out.length <= max) return out;
  const sentences = out.match(/[^.!?]+[.!?]+(\s|$)/g) ?? [];
  let kept = "";
  for (const s of sentences) {
    if (kept && kept.length + s.length > max) break;
    kept += s;
  }
  return (kept || `${out.slice(0, max).replace(/\s+\S*$/, "")}…`).trim();
}

/** The Wikipedia article a concept appears in, in the UI language first. */
export function wikipediaClaim(r: EntityResponse): { id: string; url: string | null; lang: string } | null {
  const selfIds = new Set([r.canonical_id, ...r.merged_from]);
  const found = r.claims.filter((c) => c.relation === "appears_in" && roleOf(c, selfIds) === "subject")
    .flatMap((c) => c.participants.filter((p) => p.role === "object" && p.entity.startsWith("wp:")).map((p) => {
      const prov = c.provenance.find((x) => x.source_id.startsWith("src:wikipedia-"));
      return { id: p.entity, url: prov?.locator ?? null, lang: prov?.source_id.slice("src:wikipedia-".length) ?? "" };
    }));
  return found.find((w) => w.lang === getLang()) ?? found[0] ?? null;
}

const sourceName = (r: EntityResponse, src: string): string => {
  const title = r.sources[src]?.title ?? sourceTitle(src);
  return isExplore() && /mathlib/i.test(title) ? t("codeSource") : title;
};

const emptyState = (text: string): HTMLElement => h("p", { class: "empty-state" }, text);

function question(key: string, ...body: (Node | null)[]): HTMLElement {
  return h("section", { class: "q5", "data-q": key }, h("h3", {}, t(`q_${key}`)), ...body);
}

/** «Qué es»: the sourced descriptions (each labelled with its source) and the Wikipedia extract. */
function whatIs(r: EntityResponse, actions: PanelActions): HTMLElement {
  const e = r.entity;
  const src = e.provenance[0]?.source_id ?? "";
  const body: HTMLElement[] = [];
  const desc = localized(e.descriptions);
  if (desc) {
    body.push(h("div", { class: "sourced" },
      h("p", { class: "source-label" }, fromWikidata(e) ? t("descShortWikidata") : t("descFrom", { src: sourceName(r, src) })),
      h("p", { class: "lede", lang: desc.lang }, richText(desc.text))));
  }
  const form = localized(e.statements);
  if (form) body.push(h("div", { class: "sourced" }, h("p", { class: "source-label" }, t("statementFrom", { src: sourceName(r, src) })), h("p", { lang: form.lang }, richText(form.text))));
  // Shown as the source gives it, labelled with the source: never explained by us.
  if (e.formula) body.push(h("div", { class: "sourced" }, h("p", { class: "source-label" }, fromWikidata(e) ? t("formulaWikidata") : t("formulaFrom", { src: sourceName(r, src) })), renderFormula(e.formula)));
  const first = wikipediaClaim(r);
  // The article claim can sit past the first page (número primo: 545 «incluye» first).
  // ponytail: one more page of up to 1000 claims; a server filter `relation=appears_in` removes it.
  const missed = !first && r.truncated && (r.counts_by_relation.appears_in ?? 0) > 0 && actions.claimsFrom;
  const found = first ? Promise.resolve(first) : missed ? actions.claimsFrom!(r.claims.length).then(wikipediaClaim) : null;
  if (found && actions.extract) {
    const extract = actions.extract;
    const host = h("div", { class: "sourced wiki-extract" }, h("p", { class: "muted small" }, t("loading")));
    body.push(host);
    found.then(async (wiki) => {
      if (!wiki) return host.remove();
      const text = localized((await extract(wiki.id)).entity.descriptions);
      if (!text) return host.remove();
      host.replaceChildren(
        h("p", { class: "source-label" }, `${t("wikipediaExtract")} (${wiki.lang})`),
        h("p", { lang: text.lang }, cleanExtract(text.text)),
        h("p", { class: "small" }, safeLink(wiki.url, t("readWikipedia")), h("span", { class: "license" }, " · CC BY-SA 4.0")));
    }).catch(() => host.remove());
  }
  // Nothing here is a reviewed explanation: say so, above whatever the sources give.
  return question("what", emptyState(t("noReviewed")), ...body);
}

/** «Por qué importa»: documented uses (applied in, models), each with its seal. */
function whyMatters(r: EntityResponse, selfIds: Set<string>, actions: PanelActions): HTMLElement {
  const uses = r.claims.filter((c) => (c.relation === "applied_in" || c.relation === "models") && roleOf(c, selfIds) === "subject" && !(isExplore() && isTechnicalClaim(c)));
  if (!uses.length) return question("why", emptyState(t("noReviewed")));
  return question("why", emptyState(t("noReviewed")), h("p", { class: "small" }, t("whyUses")),
    h("ul", { class: "claims" }, dedupRows(uses, selfIds).map((row) => sentenceRow(row, r, actions))));
}

/** «Qué saber antes»: only `prerequisite_for` (into the entity) and `depends_on_logically` (out of it). */
export function prerequisitesOf(r: EntityResponse): ClaimJson[] {
  const selfIds = new Set([r.canonical_id, ...r.merged_from]);
  return r.claims.filter((c) => {
    const role = roleOf(c, selfIds);
    return (c.relation === "prerequisite_for" && role !== "subject") || (c.relation === "depends_on_logically" && role === "subject");
  }).filter((c) => !(isExplore() && isTechnicalClaim(c)));
}

function knowBefore(r: EntityResponse, selfIds: Set<string>, actions: PanelActions): HTMLElement {
  const pre = prerequisitesOf(r);
  const note = h("p", { class: "muted small" }, t("beforeNote"));
  if (!pre.length) return question("before", emptyState(t("beforeEmpty")), note);
  return question("before", note, h("ul", { class: "claims" }, dedupRows(pre, selfIds).map((row) => sentenceRow(row, r, actions))));
}

const capitalize = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1);

/** «Con qué se conecta»: conflicts first (both directions recorded), then the recorded
 *  connections and the candidates, grouped by the verb read from the entity's side.  Explore
 *  shows one sentence per relation and destination; technical shows every claim. */
function connectsTo(r: EntityResponse, selfIds: Set<string>, actions: PanelActions): HTMLElement {
  const sec = question("links");
  const explore = isExplore();
  const claims = explore ? r.claims.filter((c) => !isTechnicalClaim(c)) : r.claims;
  const conflicts = conflictingClaims(claims, selfIds);
  const rest = claims.filter((c) => !conflicts.has(c.id));
  const established = rest.filter((c) => !isCandidate(c));
  const candidates = rest.filter(isCandidate);
  const conflictTag = (): HTMLElement => h("span", { class: "tag warn" }, `! ${t("conflictTag")}`);
  const rows = (list: ClaimJson[], tag?: () => HTMLElement): HTMLElement[] => explore
    ? dedupRows(list, selfIds).map((row) => sentenceRow(row, r, actions, tag?.() ?? null))
    : list.map((c) => {
      const li = claimRow(c, r.canonical_id, selfIds, actions);
      if (tag) {
        li.className += " conflict";
        li.append(tag());
      }
      return li;
    });
  if (conflicts.size) {
    // Both directions side by side, so the reader sees the pair.
    const other = (c: ClaimJson): string => othersOf(c, selfIds)[0]?.entity ?? "";
    const list = claims.filter((c) => conflicts.has(c.id)).sort((a, b) => other(a).localeCompare(other(b)));
    sec.append(h("div", { class: "conflict-block" }, h("h4", {}, t("conflictTitle")),
      h("p", { class: "notice conflict-note", role: "note" }, t("conflictNote")),
      h("ul", { class: "claims" }, rows(list, conflictTag))));
  }
  const group = (title: string, list: ClaimJson[], note?: string): void => {
    if (!list.length) return;
    // Explore groups by the verb read from the entity's side; technical by relation, as before.
    const keyOf = (c: ClaimJson): string => explore ? `${c.relation}${roleOf(c, selfIds) === "subject" ? "" : "~"}` : c.relation;
    const groups = new Map<string, ClaimJson[]>();
    for (const c of list) groups.set(keyOf(c), [...(groups.get(keyOf(c)) ?? []), c]);
    sec.append(h("h4", {}, title));
    if (note) sec.append(h("p", { class: "muted small" }, note));
    const built = [...groups].map(([key, cs]) => ({ key, cs, items: rows(cs) })).sort((a, b) => b.items.length - a.items.length);
    for (const { key, cs, items } of built) {
      const heading = explore ? capitalize(plainVerb(key.replace(/~$/, ""), key.endsWith("~"))) : relLabel(key);
      const states = new Map<string, number>();
      for (const c of cs) states.set(c.evidence_state, (states.get(c.evidence_state) ?? 0) + 1);
      const summary = explore ? null : h("span", { class: "muted small" }, ` · ${[...states].map(([s, n]) => `${n} ${stateLabel(s)}`).join(", ")}`);
      sec.append(h("details", { open: items.length <= 12 },
        h("summary", {}, h("strong", {}, heading), " ", h("span", { class: "count" }, `${items.length}`), summary),
        h("ul", { class: "claims" }, items)));
    }
  };
  // «Registradas», not «establecidas»; when none is reviewed, said in one line (C4-07).
  group(t("established"), established, established.some(isReviewed) ? undefined : t("allUnreviewed"));
  group(explore ? sealName("candidate") : t("candidates"), candidates, t("candidatesNote"));
  if (!claims.length) sec.append(emptyState(t("linksEmpty")));
  if (r.truncated) {
    sec.append(h("div", { class: "load-more" },
      h("p", { class: "muted" }, t("claimsTruncated", { n: r.claims.length.toLocaleString() })),
      actions.loadMore ? h("button", { type: "button", class: "load-more-btn", onclick: () => actions.loadMore?.() }, t("loadMore")) : null));
  }
  return sec;
}

/** «Cómo compruebo que lo entendí»: there is no reviewed exercise in the registry yet. */
const checkUnderstanding = (): HTMLElement => question("check", emptyState(t("noReviewed")), h("p", { class: "muted small" }, t("checkEmpty")));

export function renderEntity(r: EntityResponse, actions: PanelActions): HTMLElement {
  const e = r.entity;
  const explore = isExplore();
  const selfIds = new Set([r.canonical_id, ...r.merged_from]);
  const root = h("article", { class: "entity", "aria-labelledby": "entity-title" });
  root.append(
    h("header", {},
      h("p", { class: "eyebrow" }, h("span", { class: "type-dot", style: `background:${typeColor(e.type, document.documentElement.dataset.theme === "dark")}`, "aria-hidden": "true" }), typeLabel(e.type)),
      h("h2", { id: "entity-title", tabindex: -1 }, r.label),
    ),
    h("div", { class: "actions" },
      h("button", { type: "button", onclick: () => actions.showOnMap(r.canonical_id) }, t("showOnMap")),
      h("button", { type: "button", onclick: () => actions.focusRegion(r.canonical_id) }, t("focusRegion")),
      h("button", { type: "button", onclick: () => actions.pathFrom(r.canonical_id, r.label) }, t("pathFrom")),
      h("button", { type: "button", onclick: () => actions.pathTo(r.canonical_id, r.label) }, t("pathTo")),
    ),
  );
  if (r.coverage === "unknown") root.append(h("p", { class: "notice unknown-note", role: "note" }, t("coverageUnknown")));
  // The five questions, always present and always in this order.
  root.append(whatIs(r, actions), whyMatters(r, selfIds, actions), knowBefore(r, selfIds, actions), connectsTo(r, selfIds, actions), checkUnderstanding());

  const ci = r.concept;
  if (ci?.as_concept) {
    root.append(explore
      ? h("p", { class: "concept-link" }, `${t("codeBacking")} `, h("button", { type: "button", class: "mini", onclick: () => actions.openConcept(r.canonical_id) }, t("drillDown")))
      : h("p", { class: "concept-link" }, t("groupsDecls", { n: ci.as_concept.members.toLocaleString() }), " ",
        h("button", { type: "button", class: "mini", onclick: () => actions.openConcept(r.canonical_id) }, t("viewConcept"))));
  }
  if (ci?.primary_concept && !explore) {
    const pc = ci.primary_concept;
    root.append(h("p", { class: "concept-link" }, `${t("conceptOf")}: `,
      h("button", { type: "button", class: "link", title: pc.id, onclick: () => actions.openConcept(pc.id) }, pc.label + (pc.derived ? ` (${t("derivedShort")})` : ""))));
  }
  if (e.aliases.length) root.append(h("section", { class: "meta-list" }, h("p", {}, `${t("aliases")}: `, e.aliases.map(([lang, text]) => `${text} (${lang})`).join(", "))));

  const assumptions = new Set<string>();
  const limits = new Set<string>();
  for (const c of r.claims) {
    for (const a of c.assumptions) {
      const k = extractionLimit(a);
      if (k) limits.add(k);
      else assumptions.add(a);
    }
  }
  if (assumptions.size) root.append(h("section", {}, h("h3", {}, t("assumptions")), h("ul", {}, [...assumptions].map((a) => h("li", {}, richText(a))))));
  if (limits.size && !explore) root.append(h("section", {}, h("h3", {}, t("verification")), h("ul", {}, [...limits].map((k) => h("li", {}, t(k))))));

  const sources = Object.values(r.sources).filter((s) => !(explore && /mathlib/i.test(`${s.id} ${s.title}`)));
  if (sources.length) {
    root.append(h("section", {}, h("h3", {}, t("sources")),
      h("ul", { class: "sources" }, sources.map((s) => h("li", {},
        safeLink(s.uri, s.title || s.id), " ",
        h("span", { class: "license" }, `${t("license")}: ${s.license}`),
        // Version and terms are recorded in English: technical mode only (H09).
        !explore && s.version ? h("span", { class: "muted" }, ` · v${s.version}`) : null,
        !explore && s.terms ? h("div", { class: "muted small" }, s.terms) : null,
      )))));
  }
  if (!explore) {
    // Ids are for checking, not for reading: folded at the end of the record.
    root.append(h("details", { class: "identifiers" },
      h("summary", {}, t("identifiers")),
      h("p", { class: "id" }, e.id),
      r.merged_from.length ? h("p", { class: "muted" }, `${t("mergedFrom")}: ${r.merged_from.join(", ")}`) : null,
      e.external_ids.length
        ? h("div", {}, h("p", { class: "small" }, `${t("externalIds")} `, h("span", { class: "count" }, `(${e.external_ids.length})`)),
          h("p", { class: "prov wrap" }, e.external_ids.map(([s, v]) => `${s}:${v}`).join(" · ")))
        : null));
  }
  return root;
}
