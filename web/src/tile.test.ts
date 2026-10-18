// The golden file is written by core/atlas-server/tests/tile_format.rs: both sides
// of the binary tile v1 contract are tested against the same bytes.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { centralOf, decodeTile, KIND_CLUSTER, KIND_ENTITY, NONE, NO_TYPE } from "./tile.ts";

const golden = (): ArrayBuffer => {
  const b = readFileSync(new URL("../tests/fixtures/tile-v1.bin", import.meta.url));
  return b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength) as ArrayBuffer;
};

test("decodes the golden tile written by the Rust encoder", () => {
  const t = decodeTile(golden());
  assert.deepEqual([t.z, t.x, t.y, t.maxZoom], [1, 1, 0, 7]);
  assert.deepEqual([...t.bbox], [0.5, 0, 1, 0.5]);
  assert.equal(t.layer, 0);
  assert.equal(t.layoutVersion, "lv-test");
  assert.equal(t.nNodes, 2);
  assert.equal(t.nItems, 3);
  const s = (i: number): string => t.strings[i];
  assert.equal(s(t.id[0]), "L0:3");
  assert.equal(s(t.label[0]), "Geometría euclidiana");
  assert.equal(t.kind[0], KIND_CLUSTER);
  assert.equal(t.type[0], NO_TYPE);
  assert.equal(t.priority[0], NONE);
  assert.deepEqual(centralOf(t, 0), ["s:pythagoras", "c:triangle"]);
  assert.equal(s(t.label[1]), "Teorema de Pitágoras");
  assert.equal(t.kind[1], KIND_ENTITY);
  assert.equal(t.type[1], 3); // statement
  assert.equal(t.priority[1], 0);
  assert.equal(s(t.label[2]), "Álgebra");
  assert.ok(Math.abs(t.px[1] - 0.625) < 1e-7 && Math.abs(t.py[2] - 0.75) < 1e-7);
  // L0:3 -> L0:7 generalizes x12 (extracted); s:pythagoras -> L0:3 cites (disputed)
  assert.equal(t.nEdges, 2);
  assert.deepEqual([t.edgeSrc[0], t.edgeDst[0], t.edgeCount[0], t.edgeRel[0], t.edgeState[0]], [0, 2, 12, 3, 0]);
  assert.deepEqual([t.edgeSrc[1], t.edgeDst[1], t.edgeRel[1], t.edgeState[1]], [1, 0, 9, 4]);
  // aggregate of L0:3: 18 members, 17 hidden; depends_on_logically 2, cites 4
  assert.equal(t.nAggs, 1);
  assert.deepEqual([t.aggItem[0], t.aggMembers[0], t.aggHidden[0]], [0, 18, 17]);
  assert.deepEqual([...t.aggRel], [0, 9]);
  assert.deepEqual([...t.aggRelCount], [2, 4]);
  assert.deepEqual([...t.hiddenRel], [15]);
  assert.deepEqual([...t.hiddenCount], [9]);
});

test("rejects foreign, truncated and over-long tiles", () => {
  const g = golden();
  assert.throws(() => decodeTile(new ArrayBuffer(100)), /ATT1/);
  assert.throws(() => decodeTile(g.slice(0, g.byteLength - 8)), /truncated/);
  const longer = new Uint8Array(g.byteLength + 4);
  longer.set(new Uint8Array(g));
  assert.throws(() => decodeTile(longer.buffer), /trailing/);
});
