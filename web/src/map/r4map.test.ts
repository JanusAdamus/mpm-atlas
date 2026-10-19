// Round 4 map rules (docs/plans/2026-10-19-ronda-4-diseno.md): label budget and no
// overlapping label boxes at z0-z3, no jargon in explore, gold only for validated,
// neighbor ranking, camera motion tokens.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  isLibraryId, LABEL_PAD, labelBudget, labelHeight, labelTier, placeLabels, tierStyle,
  type LabelCandidate, type LabelTier, type PlacedLabel, type TierInput,
} from "./labels.ts";
import { edgeAlpha, edgeStateColors, NEUTRAL_EDGE, rankNeighbors, relationRank } from "./style.ts";
import { Camera, cubicBezier } from "./camera.ts";
import { libraryItems } from "./tiles.ts";
import { gpuTheme } from "../theme.ts";
import { EASE, MAP_LABELS, MOTION, PALETTE, rgb, type ThemeName } from "../tokens.ts";
import { EVIDENCE_STATES, RELATION_TYPES } from "../vocab.ts";
import type { Tile } from "../tile.ts";

/** Deterministic pseudo-random in [0, 1). */
const rng = (seed: number) => (): number => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);

type Kind = LabelTier | "focus" | "neighbor";

/** Width of a label as the canvas measures it, approximated: em per char + tracking. */
function approxWidth(len: number, kind: Kind, z: number): number {
  const s = kind === "focus" || kind === "neighbor" ? MAP_LABELS[kind] : tierStyle(kind, z);
  return Math.ceil(len * s.size * (s.smallCaps ? 0.62 : 0.52) + len * s.trackingEm * s.size);
}

/** A dense synthetic view: 120 areas and 3000 concepts clustered like the real layout,
 *  plus a focus with 7 neighbors pinned first (what map.ts drawOverlay builds). */
function viewCandidates(z: number, width: number, height: number, seed: number): { cands: LabelCandidate[]; pinned: number } {
  const r = rng(seed);
  const cands: LabelCandidate[] = [];
  const add = (kind: Kind, x: number, y: number, len: number): void => {
    const style = kind === "focus" || kind === "neighbor" ? MAP_LABELS[kind] : tierStyle(kind, z);
    cands.push({
      key: cands.length, x, y, r: 3 + r() * 10, w: approxWidth(len, kind, z), h: labelHeight(style),
      anchor: kind === "area" ? "center" : undefined, group: kind === "area" ? 0 : undefined,
    });
  };
  add("focus", width * 0.5, height * 0.5, 9);
  for (let i = 0; i < 7; i++) add("neighbor", width * (0.4 + 0.2 * r()), height * (0.4 + 0.2 * r()), 6 + Math.floor(r() * 20));
  const pinned = cands.length;
  for (let i = 0; i < 120; i++) add("area", r() * width, r() * height, 6 + Math.floor(r() * 22));
  for (let i = 0; i < 3000; i++) {
    const c = i % 40;
    const cx = ((c % 8) + 0.5) / 8, cy = (Math.floor(c / 8) + 0.5) / 5;
    add("concept", (cx + (r() - 0.5) * 0.15) * width, (cy + (r() - 0.5) * 0.2) * height, 4 + Math.floor(r() * 30));
  }
  return { cands, pinned };
}

const overlap = (a: PlacedLabel, b: PlacedLabel): boolean =>
  a.x < b.x + b.w + LABEL_PAD && b.x < a.x + a.w + LABEL_PAD && a.y < b.y + b.h + LABEL_PAD && b.y < a.y + a.h + LABEL_PAD;

for (const [width, height] of [[1440, 820], [390, 700]]) {
  for (let z = 0; z <= 3; z++) {
    test(`z${z} at ${width}x${height}: within the label budget, 0 overlapping label boxes`, () => {
      for (const seed of [1, 7, 42]) {
        const { cands, pinned } = viewCandidates(z, width, height, seed);
        const budget = labelBudget(z, width, height);
        const placed = placeLabels(cands, width, height, budget.total + pinned, [budget.areas]);
        assert.ok(placed.length <= budget.total + pinned, `${placed.length} labels`);
        const areas = placed.filter((p) => cands[p.key].group === 0).length;
        assert.ok(areas <= budget.areas, `${areas} areas`);
        assert.ok(areas > 0, "continents are lettered");
        assert.ok(placed.some((p) => p.key === 0), "the focus label is always placed");
        for (let i = 0; i < placed.length; i++) {
          const a = placed[i];
          assert.ok(a.x >= 0 && a.y >= 0 && a.x + a.w <= width && a.y + a.h <= height, "inside the viewport");
          for (let j = i + 1; j < placed.length; j++) assert.ok(!overlap(a, placed[j]), `boxes ${a.key} and ${placed[j].key} overlap at z${z}`);
        }
      }
    });
  }
}

test("the initial view (z0-z2, desktop and phone) letters at most 30 names", () => {
  for (const z of [0, 1, 2]) for (const [w, h] of [[1440, 820], [390, 700]]) assert.ok(labelBudget(z, w, h).total <= 30);
});

test("area names are centered on their point first (continents), others to the right", () => {
  const [area] = placeLabels([{ key: 0, x: 200, y: 100, r: 4, w: 80, h: 19, anchor: "center" }], 400, 300, 5);
  assert.equal(area.x, 160);
  const [city] = placeLabels([{ key: 0, x: 200, y: 100, r: 4, w: 80, h: 18 }], 400, 300, 5);
  assert.ok(city.x > 200);
});

const item = (kind: TierInput["kind"], type: string, id: string, label: string): TierInput => ({ kind, type, id, label });

test("explore letters no derived concept, no Lean name, no id; technical letters them", () => {
  const hidden = [
    item("concept", "concept", "concept:Mathlib.Algebra.Group.Hom", "algebra › hom (Mathlib, derivado)"),
    item("entity", "statement", "mathlib:43d573af6bb6604e", "Partition.ne_bot_of_mem'"),
    item("entity", "definition", "mathlib:Alexandrov.projSup", "Alexandrov.projSup"),
    item("entity", "area", "mathlib-dir:Mathlib/SetTheory/ZFC", "Mathlib/SetTheory/ZFC"),
    item("entity", "theory", "mathlib-mod:Mathlib.Data.List.Zip", "Mathlib.Data.List.Zip"),
    item("cluster", "", "L2:9", "Mathlib/Algebra/Polynomial"),
    item("entity", "concept", "wd:Q999999", "Q999999"),
    item("entity", "concept", "wd:Q1", "wd:Q1"),
  ];
  for (const x of hidden) {
    assert.equal(labelTier(x, "explore"), null, x.label);
    assert.notEqual(labelTier(x, "technical"), null, `${x.label} in technical`);
  }
  assert.equal(labelTier(item("entity", "statement", "mathlib:x", "Finset.sum_comm"), "technical"), "statement");
  assert.equal(labelTier(item("concept", "area", "wd:Q12479", "teoría de números"), "explore"), "area");
  assert.equal(labelTier(item("entity", "area", "wd:Q8087", "geometría"), "explore"), "area");
  assert.equal(labelTier(item("concept", "concept", "wd:Q80091", "integración"), "explore"), "concept");
  assert.equal(labelTier(item("concept", "concept", "wd:Q29175", "derivada"), "explore"), "concept", "the concept, not the jargon");
  assert.equal(labelTier(item("cluster", "", "L2:117", "meridiano"), "explore"), "subarea");
  assert.equal(labelTier(item("entity", "source_fragment", "wp:1", "Wikipedia (en): Kilogram"), "technical"), null);
  assert.ok(isLibraryId("mathlib:abc") && isLibraryId("lean-mod:Lean.Meta") && !isLibraryId("wd:Q1"));
});

test("areas in letter-spaced small caps, concepts in plain serif, declarations in italic sans", () => {
  const area = tierStyle("area", 0);
  assert.ok(area.smallCaps && area.trackingEm >= 0.1 && area.family === "serif");
  assert.equal(tierStyle("area", 4), MAP_LABELS.areaDeep, "areas recede at deep zoom");
  assert.ok(!tierStyle("concept", 2).smallCaps && tierStyle("concept", 2).family === "serif");
  assert.ok(tierStyle("statement", 5).italic && tierStyle("statement", 5).family === "ui");
  assert.equal(MAP_LABELS.focus.size, 17);
});

const key = (c: ArrayLike<number>): string => Array.from(c, (v) => v.toFixed(3)).join(",");
const gold = (th: ThemeName): string[] => [key(rgb(th, "accent-solid")), key(rgb(th, "accent-text"))];
const colorsOf = (table: Float32Array): string[] => Array.from({ length: table.length / 3 }, (_, i) => key(table.subarray(i * 3, i * 3 + 3)));

test("explore: no background edge is gold, whatever its evidence state (initial view included)", () => {
  for (const th of ["light", "dark"] as ThemeName[]) {
    const theme = gpuTheme(th);
    // The theme paints validated links gold: that is what explore must not show in the background.
    const fl = EVIDENCE_STATES.indexOf("formally_linked");
    assert.ok(gold(th).includes(colorsOf(theme.stateColor)[fl]), "precondition: formally_linked is gold in the theme");
    const explore = colorsOf(edgeStateColors(th, "explore", theme.stateColor));
    assert.equal(explore.length, 8);
    for (const c of explore) {
      assert.ok(!gold(th).includes(c), `gold background edge in ${th}`);
      assert.equal(c, key(rgb(th, NEUTRAL_EDGE)));
    }
    assert.equal(edgeStateColors(th, "technical", theme.stateColor), theme.stateColor);
  }
});

test("technical edges are fainter than before round 4 (0.5 light / 0.4 dark), so validated links make no haze", () => {
  for (const n of [10, 900, 20_000]) {
    assert.ok(edgeAlpha("technical", false, n) <= 0.5 * 0.5 + 1e-9);
    assert.ok(edgeAlpha("technical", true, n) <= 0.4 * 0.5 + 1e-9);
    assert.ok(edgeAlpha("explore", false, n) < 0.5);
  }
});

test("edges between two library nodes are flagged (explore does not draw them)", () => {
  const ids = ["mathlib:a", "mathlib:b", "wd:Q80091"];
  const tile = { nItems: 3, strings: ids, id: Uint32Array.from([0, 1, 2]) } as unknown as Tile;
  assert.deepEqual([...libraryItems(tile)], [1, 1, 0]);
});

test("neighbors: at most 7, mathematical relations before area, history and citations", () => {
  const code = (r: string): number => RELATION_TYPES.indexOf(r);
  assert.ok(relationRank(code("special_case_of")) < relationRank(code("part_of_area")));
  assert.ok(relationRank(code("part_of_area")) < relationRank(code("cites")));
  const cands = [
    { id: "cite-big", rel: code("cites"), count: 900 },
    { id: "area", rel: code("part_of_area"), count: 50 },
    { id: "gen", rel: code("generalizes"), count: 20 },
    { id: "gen", rel: code("cites"), count: 1000 },
    ...Array.from({ length: 10 }, (_, i) => ({ id: `eq${i}`, rel: code("equivalent_to"), count: i })),
  ];
  const out = rankNeighbors(cands);
  assert.equal(out.length, 7);
  assert.equal(new Set(out).size, 7);
  assert.equal(out[0], "gen");
  assert.equal(out[1], "eq9");
  assert.ok(out.includes("gen") && !out.includes("cite-big") && !out.includes("area"));
});

test("camera flights use the motion tokens; a jump with ms 0 or under reduced motion", () => {
  const ease = cubicBezier(...EASE.inOut);
  assert.equal(ease(0), 0);
  assert.equal(ease(1), 1);
  assert.ok(ease(0.25) < 0.25 && ease(0.75) > 0.75, "slow in, slow out");
  let prev = 0;
  for (let u = 0.05; u <= 1; u += 0.05) {
    assert.ok(ease(u) >= prev - 1e-9, "monotone");
    prev = ease(u);
  }
  assert.ok(MOTION.camera > 0 && MOTION.camera <= 900);
  const cam = new Camera();
  cam.resize(800, 600);
  cam.flyTo({ cx: 0.2, cy: 0.3, scale: 2000 }, 0);
  assert.ok(!cam.animating && cam.cx === 0.2 && cam.cy === 0.3);
  cam.flyTo({ cx: 0.6, cy: 0.6, scale: 2000 });
  assert.ok(cam.animating, "a chosen destination flies");
  const g = globalThis as { matchMedia?: unknown };
  const saved = g.matchMedia;
  g.matchMedia = (q: string) => ({ matches: q.includes("reduce") });
  try {
    cam.flyTo({ cx: 0.4, cy: 0.4, scale: 1500 });
    assert.ok(!cam.animating, "reduced motion: no flight");
    assert.equal(cam.view().cx, 0.4);
  } finally {
    g.matchMedia = saved;
  }
});

test("the map palette has no blue or sepia left", () => {
  for (const th of ["light", "dark"] as ThemeName[]) for (const v of Object.values(PALETTE[th])) assert.ok(!/1f56b8|7a5a2a/i.test(v));
});
