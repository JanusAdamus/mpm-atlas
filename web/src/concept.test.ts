// Reader-facing rules of the concept layer (docs/plans/2026-10-17-capa-conceptual.md):
// candidates never share a heading with established claims, extraction limits are not
// hypotheses, zone fallbacks read as low confidence, and concepts draw as their own shape.
import { test } from "node:test";
import assert from "node:assert/strict";
import { assignmentStatus, FALLBACK_SIGNALS } from "./conceptpanel.ts";
import { uiKeys } from "./i18n.ts";
import { tileInstances } from "./map/tiles.ts";
import { extractionLimit, isCandidate } from "./panel.ts";
import { KIND_CLUSTER, KIND_CONCEPT, KIND_ENTITY, NONE, type Tile } from "./tile.ts";

test("extraction limits are verification status, not hypotheses", () => {
  assert.equal(extractionLimit("axioms not computed (source import, no Lean build)"), "limit_axioms");
  assert.equal(extractionLimit("statement-level reference resolved by name from source text, without elaboration"), "limit_sigdep");
  assert.equal(extractionLimit("$f$ is continuous on $[a, b]$"), null);
});

test("model-proposed and disputed claims are candidates", () => {
  assert.ok(isCandidate({ evidence_state: "model_proposed" }));
  assert.ok(isCandidate({ evidence_state: "disputed" }));
  assert.ok(!isCandidate({ evidence_state: "extracted" }));
  assert.ok(!isCandidate({ evidence_state: "formally_linked" }));
});

test("zone fallbacks read as low confidence, model proposals as candidates (default language es)", () => {
  for (const signal of FALLBACK_SIGNALS) assert.equal(assignmentStatus({ evidence_state: "model_proposed", signal }).low, true);
  assert.deepEqual(assignmentStatus({ evidence_state: "model_proposed", signal: "name_doc" }), { text: "candidata", low: false });
  assert.deepEqual(assignmentStatus({ evidence_state: "extracted", signal: "curated" }), { text: "extraída", low: false });
});

test("every concept-layer string exists in both languages", () => {
  const es = new Set(uiKeys("es")), en = new Set(uiKeys("en"));
  const keys = ["concept", "derivedNote", "heuristicScore", "candidates", "verification", "legendConcept", "legendProximity", "noEdgesInView",
    ...["curated", "name_doc", "resolve", "namespace", "signature", "zone", "zone_derived", "unknown"].map((s) => `signal_${s}`)];
  for (const k of keys) assert.ok(es.has(k) && en.has(k), k);
  assert.deepEqual([...es].sort(), [...en].sort());
});

test("concepts, regions and entities get distinct point codes", () => {
  const u32 = (xs: number[]) => Uint32Array.from(xs);
  const t = {
    nNodes: 3, nItems: 3, nEdges: 0,
    px: Float32Array.from([0.1, 0.2, 0.3]), py: Float32Array.from([0.1, 0.2, 0.3]),
    size: u32([400, 12, 1]), priority: u32([0, NONE, 5]),
    kind: Uint8Array.from([KIND_CONCEPT, KIND_CLUSTER, KIND_ENTITY]), type: Uint8Array.from([0, 255, 3]),
    edgeSrc: u32([]), edgeDst: u32([]), edgeCount: u32([]), edgeRel: new Uint8Array(0), edgeState: new Uint8Array(0),
  } as unknown as Tile;
  const { points } = tileInstances(t);
  const code = (i: number) => points[i * 4 + 3];
  assert.equal(code(0), 0 + 1024, "concept flag on its type code");
  assert.equal(code(1), 256, "region");
  assert.equal(code(2), 3, "plain entity of its type");
  assert.ok(points[2] > points[2 * 4 + 2], "a concept with 400 members is drawn larger than an entity");
});
