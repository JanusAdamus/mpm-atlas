import { test } from "node:test";
import assert from "node:assert/strict";
import { displayLabel, placeLabels, type LabelCandidate } from "./labels.ts";
import { coveringTiles } from "./camera.ts";

const cand = (key: number, x: number, y: number, w = 60): LabelCandidate => ({ key, x, y, r: 3, w, h: 12 });

test("placed labels never overlap and keep priority order", () => {
  const cands: LabelCandidate[] = [];
  for (let i = 0; i < 2000; i++) cands.push(cand(i, 20 + ((i * 37) % 760), 20 + ((i * 91) % 560), 40 + (i % 5) * 10));
  const placed = placeLabels(cands, 800, 600, 10_000);
  assert.ok(placed.length > 20 && placed.length < 2000, `placed ${placed.length}`);
  for (let i = 0; i < placed.length; i++) {
    const a = placed[i];
    assert.ok(a.x >= 0 && a.y >= 0 && a.x + a.w <= 800 && a.y + a.h <= 600, "inside the viewport");
    for (let j = i + 1; j < placed.length; j++) {
      const b = placed[j];
      const apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
      assert.ok(apart, `labels ${a.key} and ${b.key} overlap`);
    }
  }
  assert.equal(placed[0].key, 0, "highest priority first");
});

test("a crowded spot keeps the more important label and tries other anchors", () => {
  const placed = placeLabels([cand(0, 100, 100), cand(1, 101, 100), cand(2, 100, 101)], 400, 300, 10);
  // 0 takes the right anchor, 1 falls back to the left one, 2 has no free anchor.
  assert.deepEqual(placed.map((p) => p.key), [0, 1]);
  assert.ok(placed[0].x > 100 && placed[1].x + placed[1].w < 100);
  const capped = placeLabels([cand(0, 100, 100), cand(1, 300, 200)], 400, 300, 1);
  assert.deepEqual(capped.map((p) => p.key), [0]);
});

test("covering tiles are clamped and nearest-first", () => {
  assert.deepEqual(coveringTiles(0, [-1, -1, 2, 2]), [[0, 0]]);
  const t = coveringTiles(2, [0.3, 0.3, 0.7, 0.7]);
  assert.equal(t.length, 4);
  assert.deepEqual(coveringTiles(3, [2, 2, 3, 3]), []);
  const far = coveringTiles(3, [0, 0, 1, 0.2]);
  assert.ok(far.every(([, y]) => y <= 1));
});

test("map labels drop source prefixes but keep the rest", () => {
  assert.equal(displayLabel("Wikipedia (en): Banach space"), "Banach space");
  assert.equal(displayLabel("Wikipedia (es): Número real"), "Número real");
  assert.equal(displayLabel("Mathlib/Algebra/Lie"), "Algebra/Lie");
  assert.equal(displayLabel("teorema de Pitágoras"), "teorema de Pitágoras");
  assert.equal(displayLabel("Wikipedia"), "Wikipedia");
});
