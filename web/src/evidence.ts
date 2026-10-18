// Evidence states must be told apart without color (principle 5 + accessibility):
// each state has an Okabe-Ito color (safe for the common color-vision deficiencies),
// a line pattern on the map, a line width and a glyph in badges, always next to its name.
import { stateLabel } from "./i18n.ts";
import { EVIDENCE_STATES } from "./vocab.ts";

export interface StateStyle {
  light: string;
  dark: string;
  /** Dash period in CSS px (0 = solid) and the drawn fraction of it. */
  dash: number;
  duty: number;
  width: number;
  glyph: string;
}

export const STATE_STYLE: Record<string, StateStyle> = {
  extracted: { light: "#0072B2", dark: "#56B4E9", dash: 0, duty: 1, width: 1, glyph: "○" },
  model_proposed: { light: "#8F6200", dark: "#E69F00", dash: 6, duty: 0.35, width: 1, glyph: "◌" },
  editorially_reviewed: { light: "#007A5A", dark: "#2BC79A", dash: 0, duty: 1, width: 1.6, glyph: "✓" },
  formally_linked: { light: "#A4447F", dark: "#E08CC0", dash: 0, duty: 1, width: 2, glyph: "⊢" },
  disputed: { light: "#B04A00", dark: "#FF8A3D", dash: 12, duty: 0.6, width: 1.5, glyph: "!" },
  superseded: { light: "#6E6E6E", dark: "#9A9A9A", dash: 4, duty: 0.5, width: 1, glyph: "↷" },
  retracted: { light: "#6E6E6E", dark: "#9A9A9A", dash: 2, duty: 0.5, width: 1, glyph: "✕" },
};

export const styleOf = (state: string): StateStyle => STATE_STYLE[state] ?? STATE_STYLE.extracted;

/** RGB in 0..1 for the GPU, by state code. */
export function stateRgb(code: number, dark: boolean): [number, number, number] {
  const s = styleOf(EVIDENCE_STATES[code] ?? "extracted");
  return hexRgb(dark ? s.dark : s.light);
}

export function hexRgb(hex: string): [number, number, number] {
  const v = parseInt(hex.slice(1), 16);
  return [((v >> 16) & 255) / 255, ((v >> 8) & 255) / 255, (v & 255) / 255];
}

/** `<span class="badge">` with glyph + localized name; color is never the only signal. */
export function badge(state: string): HTMLElement {
  const el = document.createElement("span");
  el.className = `badge state-${state}`;
  const s = styleOf(state);
  el.textContent = `${s.glyph} ${stateLabel(state)}`;
  el.title = stateLabel(state);
  return el;
}

/** Legend swatch: an SVG line drawn with the state's pattern and width. */
export function swatch(state: string): SVGSVGElement {
  const s = styleOf(state);
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", "36");
  svg.setAttribute("height", "10");
  svg.setAttribute("aria-hidden", "true");
  const line = document.createElementNS(ns, "line");
  line.setAttribute("x1", "1");
  line.setAttribute("x2", "35");
  line.setAttribute("y1", "5");
  line.setAttribute("y2", "5");
  line.setAttribute("class", `stroke-${state}`);
  line.setAttribute("stroke-width", String(s.width + 0.5));
  if (s.dash > 0) line.setAttribute("stroke-dasharray", `${s.dash * s.duty} ${s.dash * (1 - s.duty)}`);
  svg.append(line);
  return svg;
}
