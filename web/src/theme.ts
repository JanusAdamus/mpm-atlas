// Light/dark: CSS tokens for the DOM, the same choice mirrored into GPU tables.
// Colors come from tokens.ts (round 4): paper and ink, gold only for validated.
import type { Theme } from "./map/gl.ts";
import { EVIDENCE, PALETTE, rgb, type Palette, type ThemeName } from "./tokens.ts";
import { ENTITY_TYPES, EVIDENCE_STATES } from "./vocab.ts";

export type { ThemeName } from "./tokens.ts";

/** Entity types are told apart by name and mark shape, not by hue (one color, one meaning).
 *  Dots stay a step lighter than the ink of labels so names read on top (>= 3:1 on paper).
 *  Concepts and areas in muted ink; everything else in veiled ink. */
const TYPE_INK: Record<string, keyof Palette> = { concept: "text-muted", area: "text-muted" };
const OTHER: keyof Palette = "text-veiled";
const REGION: keyof Palette = "border-strong";

export function typeColor(type: string, dark: boolean): string {
  return PALETTE[dark ? "dark" : "light"][TYPE_INK[type] ?? OTHER];
}

export function gpuTheme(name: ThemeName): Theme {
  const palette = new Float32Array(32 * 3);
  ENTITY_TYPES.forEach((ty, i) => palette.set(rgb(name, TYPE_INK[ty] ?? OTHER), i * 3));
  palette.set(rgb(name, REGION), 31 * 3);
  const stateColor = new Float32Array(8 * 3);
  const stateDash = new Float32Array(8 * 3);
  EVIDENCE_STATES.forEach((st, i) => {
    const m = EVIDENCE[st] ?? EVIDENCE.extracted;
    stateColor.set(rgb(name, m.color), i * 3);
    stateDash.set([m.dash, m.duty, m.width], i * 3);
  });
  const paper = rgb(name, "bg-page");
  return { palette, stateColor, stateDash, outline: paper, background: paper };
}

export function initialTheme(): ThemeName {
  try {
    const saved = localStorage.getItem("atlas.theme");
    if (saved === "light" || saved === "dark") return saved;
  } catch {
    // Storage blocked (private mode): fall back to the system preference.
  }
  return matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function applyTheme(name: ThemeName): void {
  const root = document.documentElement;
  // No half-finished color transitions while the whole page changes theme.
  root.classList.add("theme-switching");
  root.dataset.theme = name;
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove("theme-switching")));
  try {
    localStorage.setItem("atlas.theme", name);
  } catch {
    // Not persisted; the choice still applies to this page.
  }
}
