// Round 4 design system: style.css and tokens.ts carry the same colors, and every declared
// pair passes WCAG 2.2 contrast (text >= 4.5:1, graphics >= 3:1) in light and dark.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { CONTRAST_PAIRS, EVIDENCE, MAP_CONTRAST_PAIRS, PALETTE, VEIL, parseColor, type ContrastPair, type ThemeName } from "./tokens.ts";

const CSS = readFileSync(new URL("./style.css", import.meta.url), "utf8");

/** Custom properties of one rule block (first match), with `var(--x)` resolved. */
export function themeVars(css: string, theme: ThemeName): Map<string, string> {
  const block = (sel: RegExp): string => {
    const m = sel.exec(css);
    if (!m) throw new Error(`no block ${sel}`);
    return css.slice(m.index + m[0].length, css.indexOf("}", m.index));
  };
  const read = (body: string, into: Map<string, string>): void => {
    for (const m of body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) into.set(m[1], m[2].trim());
  };
  const vars = new Map<string, string>();
  read(block(/:root\s*\{/), vars);
  if (theme === "dark") read(block(/:root\[data-theme="dark"\]\s*\{/), vars);
  const resolve = (v: string, depth = 0): string => {
    const m = /^var\((--[\w-]+)\)$/.exec(v);
    if (!m || depth > 8) return v;
    const next = vars.get(m[1]);
    if (next === undefined) throw new Error(`undefined ${m[1]}`);
    return resolve(next, depth + 1);
  };
  return new Map([...vars].map(([k, v]) => [k, resolve(v)]));
}

const lum = (c: string): number => {
  const [r, g, b] = parseColor(c).map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};
export const contrast = (a: string, b: string): number => {
  const [x, y] = [lum(a), lum(b)];
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};
const MIN: Record<ContrastPair["kind"], number> = { text: 4.5, graphic: 3 };

/** Failing pairs, given a name -> color lookup. */
export function failingPairs(color: (key: string) => string, list: readonly ContrastPair[] = CONTRAST_PAIRS): string[] {
  return list.flatMap((p) => {
    const r = contrast(color(p.fg), color(p.bg));
    return r < MIN[p.kind] ? [`${p.kind} ${p.fg} on ${p.bg}: ${r.toFixed(2)} < ${MIN[p.kind]}`] : [];
  });
}

for (const theme of ["light", "dark"] as const) {
  const vars = themeVars(CSS, theme);
  const css = (key: string): string => {
    const v = vars.get(`--color-${key}`);
    if (v === undefined) throw new Error(`style.css lacks --color-${key}`);
    return v;
  };

  test(`${theme}: style.css and tokens.ts declare the same colors`, () => {
    for (const [key, value] of Object.entries(PALETTE[theme])) {
      assert.equal(css(key).toLowerCase().replace(/\s+/g, ""), value.toLowerCase().replace(/\s+/g, ""), `--color-${key}`);
    }
  });

  test(`${theme}: every declared pair meets WCAG contrast`, () => {
    assert.deepEqual(failingPairs(css), []);
  });

  test(`${theme}: the map under a focus or route: labels >= 4.5:1, points >= 3:1 (C4-08)`, () => {
    assert.deepEqual(failingPairs(css, MAP_CONTRAST_PAIRS), []);
    const kinds = MAP_CONTRAST_PAIRS.map((p) => `${p.fg}:${p.kind}`);
    assert.ok(kinds.includes(`${VEIL.label}:text`) && kinds.includes(`${VEIL.point}:graphic`), "the veiled label and point are covered");
    // Before this pass the veiled labels were painted in text-veiled (3.78 light, 3.95 dark).
    assert.ok(contrast(css("text-veiled"), css("bg-page")) < 4.5);
  });

  test(`${theme}: evidence badge text is legible and strokes are visible`, () => {
    for (const [state, m] of Object.entries(EVIDENCE)) {
      for (const bg of ["bg-page", "bg-surface"]) {
        assert.ok(contrast(css(m.text), css(bg)) >= 4.5, `${state} text on ${bg}`);
        assert.ok(contrast(css(m.color), css(bg)) >= 3, `${state} stroke on ${bg}`);
      }
    }
  });
}

test("gold means validated only: no other evidence state uses an accent token", () => {
  for (const [state, m] of Object.entries(EVIDENCE)) {
    const gold = m.color.startsWith("accent") || m.text.startsWith("accent");
    assert.equal(gold, state === "editorially_reviewed" || state === "formally_linked", state);
  }
});

test("validated and candidate never share a stroke pattern", () => {
  assert.equal(EVIDENCE.editorially_reviewed.dash, 0);
  assert.ok(EVIDENCE.model_proposed.dash > 0);
});

test("the contrast check fails on a known failing pair", () => {
  // Old light theme: --line-strong #cfccc2 as a control border on --panel #fdfdfb.
  assert.ok(contrast("#cfccc2", "#fdfdfb") < 3);
  assert.equal(contrast("#000000", "#ffffff").toFixed(0), "21");
});

test("no HTML text below the 12 px floor in style.css", () => {
  const small = [...CSS.matchAll(/font(?:-size)?\s*:[^;]*?(\d+(?:\.\d+)?)px/g)].map((m) => +m[1]).filter((px) => px < 12);
  assert.deepEqual(small, []);
});
