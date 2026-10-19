// Design tokens of round 4 ("atlas impreso", docs/design/r4/guia-skills-diseno.md §2).
// The same values live in style.css (`:root` and `[data-theme="dark"]`); tokens.test.ts
// fails if the two drift apart and checks every declared contrast pair (WCAG 2).
// One color, one meaning: gold = validated only.  Focus is ink + size + ring + veil.

export type ThemeName = "light" | "dark";

/** Semantic colors.  Keys are the CSS names without `--color-`. */
export interface Palette {
  "bg-page": string; // paper
  "bg-surface": string; // panel / ficha
  "bg-raised": string; // inputs, popovers
  "bg-hover": string; // hover / pressed neutral
  "text-primary": string; // ink
  "text-secondary": string;
  "text-muted": string;
  "text-veiled": string; // points under the focus veil (graphic, >= 3:1); never text
  "on-ink": string; // text on an ink-filled primary button
  border: string; // hairline, decorative only
  "border-strong": string; // control boundary
  "accent-text": string; // gold for text: validated
  "accent-solid": string; // gold for strokes/dots: validated (never under text)
  "accent-subtle": string;
  candidate: string; // gray, always dotted: candidate, not reviewed
  danger: string; // disputed
  "danger-subtle": string;
  unknown: string; // terra incognita fill (hatched)
  "unknown-hatch": string;
  "focus-ring": string;
  veil: string; // paper wash of the panels (the map veils by ink, see VEIL)
  halo: string; // paper halo behind map labels
}

export const PALETTE: Record<ThemeName, Palette> = {
  light: {
    "bg-page": "#f4f1e8",
    "bg-surface": "#fbfaf5",
    "bg-raised": "#ffffff",
    "bg-hover": "#ebe6d9",
    "text-primary": "#1c1b18",
    "text-secondary": "#48443b",
    "text-muted": "#655f53",
    "text-veiled": "#807a6c",
    "on-ink": "#fbfaf5",
    border: "#d6cfbf",
    "border-strong": "#8f8775",
    "accent-text": "#7a5600",
    "accent-solid": "#a87a12",
    "accent-subtle": "#f2e5c2",
    candidate: "#857e70",
    danger: "#a3361f",
    "danger-subtle": "#f7e2da",
    unknown: "#e6e0d2",
    "unknown-hatch": "#7a7466",
    "focus-ring": "#1c1b18",
    veil: "rgba(244, 241, 232, 0.6)",
    halo: "rgba(244, 241, 232, 0.94)",
  },
  dark: {
    "bg-page": "#14130f",
    "bg-surface": "#1c1a16",
    "bg-raised": "#252219",
    "bg-hover": "#2a271f",
    "text-primary": "#ece6d6",
    "text-secondary": "#c6bea9",
    "text-muted": "#9c9483",
    "text-veiled": "#7a7363",
    "on-ink": "#14130f",
    border: "#38342b",
    "border-strong": "#6e6757",
    "accent-text": "#e3b74e",
    "accent-solid": "#c99a2e",
    "accent-subtle": "#3a2f14",
    candidate: "#8a8374",
    danger: "#f08a6c",
    "danger-subtle": "#3a1f17",
    unknown: "#24211b",
    "unknown-hatch": "#807868",
    "focus-ring": "#ece6d6",
    veil: "rgba(20, 19, 15, 0.6)",
    halo: "rgba(20, 19, 15, 0.9)",
  },
};

/** Pairs that must stay legible: text >= 4.5:1, graphics (strokes, borders, rings) >= 3:1. */
export interface ContrastPair {
  fg: keyof Palette;
  bg: keyof Palette;
  kind: "text" | "graphic";
}

const pairs = (kind: ContrastPair["kind"], fgs: (keyof Palette)[], bgs: (keyof Palette)[]): ContrastPair[] =>
  fgs.flatMap((fg) => bgs.map((bg) => ({ fg, bg, kind })));

const SURFACES: (keyof Palette)[] = ["bg-page", "bg-surface", "bg-raised"];

export const CONTRAST_PAIRS: readonly ContrastPair[] = [
  ...pairs("text", ["text-primary", "text-secondary", "text-muted", "accent-text", "danger"], SURFACES),
  ...pairs("text", ["text-primary", "text-secondary", "text-muted"], ["bg-hover"]),
  ...pairs("text", ["text-primary", "accent-text"], ["accent-subtle"]),
  ...pairs("text", ["text-primary", "danger"], ["danger-subtle"]),
  ...pairs("text", ["text-primary", "text-muted"], ["unknown"]),
  { fg: "on-ink", bg: "text-primary", kind: "text" },
  ...pairs("graphic", ["border-strong", "accent-solid", "candidate", "focus-ring", "text-veiled"], ["bg-page", "bg-surface"]),
  ...pairs("graphic", ["accent-solid", "candidate", "text-secondary", "danger"], ["bg-raised"]),
  ...pairs("graphic", ["candidate", "text-secondary", "danger"], ["unknown"]), // no validated link lives in an unknown region
  { fg: "unknown-hatch", bg: "unknown", kind: "graphic" },
];

/** The map under a focus or a route (C4-08): no paper wash over points or labels, so what
 *  is drawn is what is measured.  Veiled labels keep a text ink (>= 4.5:1) and step down
 *  in size and weight (labels.ts veiledStyle); veiled points take a lighter ink (>= 3:1);
 *  background edges fade by `edgeFactor`.  Lit points (no focus) share one neutral ink:
 *  on the map a point's color means nothing, its shape says what it is (C4 color rule). */
export const VEIL = { label: "text-muted", point: "text-veiled", pointLit: "text-muted", edgeFactor: 0.4 } as const satisfies {
  label: keyof Palette; point: keyof Palette; pointLit: keyof Palette; edgeFactor: number;
};
/** What the map paints, against the paper it is painted on. */
export const MAP_CONTRAST_PAIRS: readonly ContrastPair[] = [
  { fg: VEIL.label, bg: "bg-page", kind: "text" },
  { fg: VEIL.point, bg: "bg-page", kind: "graphic" },
  { fg: VEIL.pointLit, bg: "bg-page", kind: "graphic" },
];

/** Evidence states: stroke + glyph + name, never color alone (guide §2.3). */
export interface EvidenceMark {
  color: keyof Palette; // stroke / dot
  text: keyof Palette; // badge text
  dash: number; // period in CSS px, 0 = solid
  duty: number; // drawn fraction of the period
  width: number; // CSS px
  glyph: string;
}

export const EVIDENCE: Record<string, EvidenceMark> = {
  editorially_reviewed: { color: "accent-solid", text: "accent-text", dash: 0, duty: 1, width: 2, glyph: "✓" },
  formally_linked: { color: "accent-solid", text: "accent-text", dash: 0, duty: 1, width: 2, glyph: "✓" },
  extracted: { color: "text-secondary", text: "text-secondary", dash: 0, duty: 1, width: 1, glyph: "—" },
  model_proposed: { color: "candidate", text: "text-muted", dash: 3, duty: 0.4, width: 1, glyph: "?" },
  disputed: { color: "danger", text: "danger", dash: 8, duty: 0.6, width: 1.5, glyph: "!" },
  superseded: { color: "text-muted", text: "text-muted", dash: 2, duty: 0.5, width: 1, glyph: "×" },
  retracted: { color: "text-muted", text: "text-muted", dash: 2, duty: 0.5, width: 1, glyph: "×" },
};
/** A historical connection is not an evidence state: long ink dash, glyph ◷ (a clock face,
 *  Geometric Shapes; U+29D7 ⧗ fell back to a serif «Ɪ» in Edge on Windows, C4-16). */
export const HISTORICAL: EvidenceMark = { color: "text-primary", text: "text-primary", dash: 14, duty: 0.7, width: 1.5, glyph: "◷" };
/** Terra incognita: 45° hatch, 1 px every 6 px, over `unknown`. */
export const UNKNOWN_HATCH = { angleDeg: 45, width: 1, spacing: 6 } as const;
/** Focus: the rest of the map sits under `veil`; focus dot 7 px with an ink ring. */
export const FOCUS = { dot: 4, focusDot: 7, ring: 2 } as const;

/** Map lettering (guide §2.2).  Sizes in CSS px; multiply by devicePixelRatio in WebGL. */
export interface MapLabelStyle {
  family: "serif" | "ui";
  weight: number;
  size: number;
  trackingEm: number;
  smallCaps: boolean;
  italic: boolean;
  color: keyof Palette;
}
export const MAP_LABELS: Record<"area" | "areaDeep" | "subarea" | "concept" | "focus" | "neighbor" | "statement", MapLabelStyle> = {
  area: { family: "serif", weight: 600, size: 15, trackingEm: 0.14, smallCaps: true, italic: false, color: "text-secondary" },
  areaDeep: { family: "serif", weight: 600, size: 13, trackingEm: 0.14, smallCaps: true, italic: false, color: "text-muted" },
  subarea: { family: "serif", weight: 500, size: 13, trackingEm: 0.08, smallCaps: true, italic: false, color: "text-secondary" },
  concept: { family: "serif", weight: 400, size: 14, trackingEm: 0, smallCaps: false, italic: false, color: "text-primary" },
  focus: { family: "serif", weight: 600, size: 17, trackingEm: 0, smallCaps: false, italic: false, color: "text-primary" },
  neighbor: { family: "serif", weight: 500, size: 14, trackingEm: 0, smallCaps: false, italic: false, color: "text-primary" },
  statement: { family: "ui", weight: 400, size: 12, trackingEm: 0, smallCaps: false, italic: true, color: "text-muted" },
};
export const FONT = {
  serif: '"Iowan Old Style", "Charter", "Bitstream Charter", "Sitka Text", Cambria, Georgia, serif',
  ui: 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif',
  mono: 'ui-monospace, "Cascadia Mono", Consolas, monospace',
} as const;

/** Motion (ms).  Camera flights read these; under reduced motion use REDUCED_MOTION. */
export const MOTION = { instant: 0, fast: 100, press: 150, panelIn: 200, panelOut: 150, camera: 300, routeDraw: 600, stagger: 40, staggerMax: 300 } as const;
export const REDUCED_MOTION = { ...MOTION, press: 0, camera: 0, routeDraw: 0, stagger: 0, staggerMax: 0, panelIn: 150, panelOut: 150 } as const;
/** cubic-bezier control points. */
export const EASE = { out: [0.23, 1, 0.32, 1], inOut: [0.77, 0, 0.175, 1] } as const;

export function prefersReducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** "#rrggbb" or "rgba(r, g, b, a)" to RGBA in 0..1. */
export function parseColor(c: string): [number, number, number, number] {
  const m = /^rgba?\(\s*([\d.]+)\s*,\s*([\d.]+)\s*,\s*([\d.]+)\s*(?:,\s*([\d.]+)\s*)?\)$/.exec(c.trim());
  if (m) return [+m[1] / 255, +m[2] / 255, +m[3] / 255, m[4] === undefined ? 1 : +m[4]];
  const h = /^#([0-9a-f]{6})$/i.exec(c.trim());
  if (!h) throw new Error(`unsupported color: ${c}`);
  const v = parseInt(h[1], 16);
  return [((v >> 16) & 255) / 255, ((v >> 8) & 255) / 255, (v & 255) / 255, 1];
}

/** RGB in 0..1 for the GPU. */
export function rgb(theme: ThemeName, key: keyof Palette): [number, number, number] {
  const [r, g, b] = parseColor(PALETTE[theme][key]);
  return [r, g, b];
}
