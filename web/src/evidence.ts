// Evidence seals (guide §2.3): every state has a stroke, a glyph and a plain name, never
// color alone.  Glyphs, strokes and colors come from tokens.ts (EVIDENCE, HISTORICAL);
// the Okabe-Ito palette and the old glyphs are retired.  One color, one meaning: the
// legend lists each seal once, «desconocida» (terra incognita) included.
import { h } from "./dom.ts";
import { isExplore, sealMeaning, sealName, stateLabel, t } from "./i18n.ts";
import { EVIDENCE, HISTORICAL, type EvidenceMark } from "./tokens.ts";

/** Registry state → reader-facing seal. */
const SEAL_OF: Record<string, string> = {
  editorially_reviewed: "validated",
  formally_linked: "validated",
  extracted: "documented",
  model_proposed: "candidate",
  disputed: "disputed",
  superseded: "retired",
  retracted: "retired",
};

/** The seals of the legend, in reading order; `historical` and `unknown` are not evidence states. */
export const SEALS = ["validated", "documented", "candidate", "disputed", "retired", "historical", "unknown"] as const;
export type Seal = (typeof SEALS)[number];

export const sealOf = (state: string): Seal => (SEAL_OF[state] ?? "documented") as Seal;

const STATE_OF_SEAL: Record<string, string> = { validated: "formally_linked", documented: "extracted", candidate: "model_proposed", disputed: "disputed", retired: "retracted" };

/** Stroke and glyph of a seal (unknown has a hatch, not a stroke). */
export function markOf(seal: Seal): EvidenceMark | null {
  if (seal === "historical") return HISTORICAL;
  if (seal === "unknown") return null;
  return EVIDENCE[STATE_OF_SEAL[seal]];
}

const glyphOf = (seal: Seal): string => markOf(seal)?.glyph ?? "▨";

/** `<span class="badge">`: glyph + plain name; the technical mode adds the registry state. */
export function badge(state: string): HTMLElement {
  const seal = sealOf(state);
  const name = sealName(seal);
  const tech = isExplore() ? "" : ` · ${stateLabel(state)}`;
  return h("span", { class: `badge state-${state} seal-${seal}`, title: `${name}: ${sealMeaning(seal)}` },
    h("span", { class: "glyph", "aria-hidden": "true" }, glyphOf(seal)), `${name}${tech}`);
}

/** The seal of a historical route (not an evidence state). */
export function historicalBadge(): HTMLElement {
  return h("span", { class: "badge seal-historical", title: `${sealName("historical")}: ${sealMeaning("historical")}` },
    h("span", { class: "glyph", "aria-hidden": "true" }, HISTORICAL.glyph), sealName("historical"));
}

/** Legend swatch: the seal's stroke (pattern and width), or the hatch of terra incognita. */
export function swatch(seal: Seal): Element {
  const m = markOf(seal);
  if (!m) return h("span", { class: "swatch-unknown unknown-region", "aria-hidden": "true" });
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", "28");
  svg.setAttribute("height", "10");
  svg.setAttribute("aria-hidden", "true");
  const line = document.createElementNS(ns, "line");
  line.setAttribute("x1", "1");
  line.setAttribute("x2", "27");
  line.setAttribute("y1", "5");
  line.setAttribute("y2", "5");
  line.setAttribute("class", `stroke-${seal}`);
  line.setAttribute("stroke-width", String(m.width + 0.5));
  if (m.dash > 0) line.setAttribute("stroke-dasharray", `${m.dash * m.duty} ${m.dash * (1 - m.duty)}`);
  svg.append(line);
  return svg;
}

/** Every seal once, with its swatch, glyph and name; `full` adds what it means. */
export function legendList(full: boolean): HTMLElement {
  return h("ul", { class: full ? "seal-legend full" : "seal-legend" }, SEALS.map((s) =>
    h("li", { title: sealMeaning(s) }, swatch(s), h("span", { class: "glyph", "aria-hidden": "true" }, glyphOf(s)),
      h("span", {}, sealName(s), full ? h("span", { class: "muted" }, ` — ${sealMeaning(s)}`) : null))));
}

/** Reader-facing name of a source id when only the id is at hand (routes carry no titles). */
export function sourceTitle(id: string): string {
  if (id === "src:wikidata") return "Wikidata";
  if (id.startsWith("src:wikipedia")) return "Wikipedia";
  if (/mathlib/i.test(id)) return t("codeSource");
  return id.replace(/^src:/, "");
}
