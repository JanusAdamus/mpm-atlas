// Round 4, second map pass (docs/design/r4/critica/2026-10-19-critica-r4.md, C4-01..C4-16).
// Each test fails with the code before this pass.
import { test } from "node:test";
import assert from "node:assert/strict";
import { routeHopMark, pointPalette } from "./style.ts";
import { labelTier, veiledStyle } from "./labels.ts";
import { untranslatedItems } from "./tiles.ts";
import { Camera, contentBounds } from "./camera.ts";
import { KIND_CLUSTER, KIND_ENTITY, type Tile } from "../tile.ts";
import { ENTITY_TYPES } from "../vocab.ts";
import { EVIDENCE, HISTORICAL, MAP_LABELS, PALETTE, rgb, VEIL } from "../tokens.ts";

test("C4-01/C4-02: each route hop is drawn in its own evidence state", () => {
  // A hop without a state (the concept route) is a candidate: dotted gray, never solid ink.
  assert.equal(routeHopMark(undefined, false), EVIDENCE.model_proposed);
  assert.ok(routeHopMark(undefined, false).dash > 0);
  assert.equal(routeHopMark("model_proposed", false), EVIDENCE.model_proposed);
  // A documentary (exploration) route: «Documentada» per hop, not the «Histórica» dash.
  assert.equal(routeHopMark("extracted", false), EVIDENCE.extracted);
  assert.notEqual(routeHopMark("extracted", false), HISTORICAL);
  // Validated in gold; the long dash only when the route is historical.
  assert.equal(routeHopMark("formally_linked", false).color, "accent-solid");
  assert.equal(routeHopMark("extracted", true), HISTORICAL);
});

test("C4-08: under the veil labels keep a text ink and step down in size and weight", () => {
  for (const s of Object.values(MAP_LABELS)) {
    const v = veiledStyle(s);
    assert.equal(v.color, VEIL.label);
    assert.ok(v.weight < MAP_LABELS.neighbor.weight && v.size < MAP_LABELS.neighbor.size, "below the lit neighbor");
    assert.ok(v.weight < MAP_LABELS.focus.weight && v.size < MAP_LABELS.focus.size, "below the focus");
  }
  // No map lettering uses the points' veiled ink as text.
  for (const s of Object.values(MAP_LABELS)) assert.notEqual(s.color, "text-veiled");
});

test("C4 color rule: a point's color means nothing (one neutral ink, a lighter one under the veil)", () => {
  for (const th of ["light", "dark"] as const) {
    for (const veiled of [false, true]) {
      const p = pointPalette(th, veiled);
      const c = rgb(th, veiled ? VEIL.point : VEIL.pointLit);
      for (let i = 0; i < 32; i++) assert.deepEqual([...p.subarray(i * 3, i * 3 + 3)].map((x) => +x.toFixed(4)), c.map((x) => +x.toFixed(4)));
    }
    assert.notEqual(PALETTE[th][VEIL.point], PALETTE[th][VEIL.pointLit]);
  }
});

const ty = (name: string): string => name;
test("C4-13: explore letters no person and no article title as a place; technical keeps them", () => {
  const person = { kind: "entity" as const, type: ty("person"), id: "wd:Q38636", label: "Karen Uhlenbeck" };
  const article = { kind: "cluster" as const, type: "", id: "L3:7", label: "Wikipedia (en): Temperature" };
  assert.equal(labelTier(person, "explore"), null);
  assert.equal(labelTier(article, "explore"), null);
  assert.equal(labelTier(person, "technical"), "concept");
  assert.equal(labelTier(article, "technical"), "subarea");
  assert.equal(labelTier({ kind: "entity", type: "area", id: "wd:Q1", label: "álgebra" }, "explore"), "area");
});

/** A minimal tile: nodes with id, label, kind and type. */
function fakeTile(nodes: { id: string; label: string; kind: number; type: string }[]): Tile {
  const strings: string[] = [];
  const s = (x: string): number => (strings.push(x), strings.length - 1);
  const n = nodes.length;
  return {
    strings, nNodes: n, nItems: n,
    id: Uint32Array.from(nodes.map((x) => s(x.id))),
    label: Uint32Array.from(nodes.map((x) => s(x.label))),
    kind: Uint8Array.from(nodes.map((x) => x.kind)),
    type: Uint8Array.from(nodes.map((x) => Math.max(0, ENTITY_TYPES.indexOf(x.type)))),
  } as unknown as Tile;
}

test("C4-13: areas and regions whose label is the English fallback are marked", () => {
  const es = fakeTile([
    { id: "a1", label: "knot theory", kind: KIND_ENTITY, type: "area" },
    { id: "a2", label: "teoría de grafos", kind: KIND_ENTITY, type: "area" },
    { id: "c1", label: "Ashoke Sen", kind: KIND_CLUSTER, type: "" },
    { id: "p1", label: "Fourier", kind: KIND_ENTITY, type: "person" },
  ]);
  const en = fakeTile([
    { id: "a1", label: "knot theory", kind: KIND_ENTITY, type: "area" },
    { id: "a2", label: "graph theory", kind: KIND_ENTITY, type: "area" },
    { id: "c1", label: "Ashoke Sen", kind: KIND_CLUSTER, type: "" },
    { id: "p1", label: "Fourier", kind: KIND_ENTITY, type: "person" },
  ]);
  assert.deepEqual([...untranslatedItems(es, en)], [1, 0, 1, 0]);
});

test("C4-12: the home framing follows a resize until the reader moves", () => {
  const c = new Camera();
  c.resize(390, 700);
  // The real content rectangle of the root tile (measured 2026-10-19): about 0.53 x 0.50.
  c.home = [0.234, 0.069, 0.768, 0.566];
  c.goHome(0);
  c.resize(390, 354); // the phone layout settles: the map gets 42vh
  const v = c.homeView();
  assert.equal(c.scale, v.scale);
  assert.equal(c.cx, v.cx);
  // It fills the useful width: at least 90 % of the 390 px.
  assert.ok((0.768 - 0.234) * c.scale >= 0.9 * 390, `${((0.768 - 0.234) * c.scale).toFixed(0)} px`);
  c.pan(30, 0);
  const moved = c.cx;
  c.resize(390, 400);
  assert.equal(c.cx, moved, "after a pan a resize keeps the reader's view");
  assert.ok(contentBounds([0, 1], [0, 1]));
});

test("C4-16: the «Histórica» glyph is not U+29D7 (drawn as «Ɪ» in Edge on Windows)", () => {
  assert.notEqual(HISTORICAL.glyph, "⧗");
  // Checked in headless Edge on Windows 11: drawn by Cambria Math, a clock face at 13 px.
  assert.equal(HISTORICAL.glyph, "◷");
});
