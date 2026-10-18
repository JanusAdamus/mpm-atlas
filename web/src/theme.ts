// Light/dark: CSS tokens for the DOM, the same choice mirrored into GPU tables.
import { STATE_STYLE, hexRgb } from "./evidence.ts";
import type { Theme } from "./map/gl.ts";
import { ENTITY_TYPES, EVIDENCE_STATES } from "./vocab.ts";

export type ThemeName = "light" | "dark";

/** Entity-type colors (secondary information; the type is also named in labels and panels). */
const TYPE_COLORS: Record<string, [string, string]> = {
  concept: ["#3B6FB6", "#7FA8E8"],
  definition: ["#2F8FA3", "#6CC6D8"],
  theory: ["#6A4FB0", "#A992F0"],
  statement: ["#B5473A", "#F08A7C"],
  proof: ["#8A5A2B", "#D2A06C"],
  conjecture: ["#B0306A", "#EE7BB0"],
  counterexample: ["#7A2E2E", "#D27A7A"],
  algorithm: ["#2E7D4F", "#6FCB93"],
  model: ["#4C7A2E", "#9AC970"],
  problem: ["#A0602A", "#E7A465"],
  phenomenon: ["#527A8C", "#93C1D6"],
  application: ["#3E8E6E", "#7DD3AF"],
  publication: ["#707070", "#B0B0B0"],
  source_fragment: ["#808080", "#A8A8A8"],
  person: ["#9C6B00", "#E6B23C"],
  resource: ["#5E6A7A", "#A3AFBF"],
  experiment: ["#5B8A3A", "#A2D07C"],
  area: ["#384A80", "#8FA2DE"],
};
const REGION: [string, string] = ["#55607A", "#8C98B5"];

export function typeColor(type: string, dark: boolean): string {
  const c = TYPE_COLORS[type] ?? REGION;
  return dark ? c[1] : c[0];
}

export function gpuTheme(name: ThemeName): Theme {
  const dark = name === "dark";
  const palette = new Float32Array(32 * 3);
  ENTITY_TYPES.forEach((ty, i) => palette.set(hexRgb(typeColor(ty, dark)), i * 3));
  palette.set(hexRgb(dark ? REGION[1] : REGION[0]), 31 * 3);
  const stateColor = new Float32Array(8 * 3);
  const stateDash = new Float32Array(8 * 3);
  EVIDENCE_STATES.forEach((st, i) => {
    const s = STATE_STYLE[st];
    stateColor.set(hexRgb(dark ? s.dark : s.light), i * 3);
    stateDash.set([s.dash, s.duty, s.width], i * 3);
  });
  return {
    palette,
    stateColor,
    stateDash,
    outline: dark ? [0.059, 0.067, 0.078] : [1, 1, 1],
    background: dark ? [0.059, 0.067, 0.078] : [0.965, 0.961, 0.945],
  };
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
  document.documentElement.dataset.theme = name;
  try {
    localStorage.setItem("atlas.theme", name);
  } catch {
    // Not persisted; the choice still applies to this page.
  }
}
