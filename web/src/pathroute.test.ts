// Concept-first answers (round 3): the web uses PathResponse.concepts, draws the concept
// route when there is no declaration-level one, and every new string has both languages.
import { test } from "node:test";
import assert from "node:assert/strict";
import type { Conclusion, ConceptLevel, PathResponse } from "./api.ts";
import { uiKeys } from "./i18n.ts";
import { conceptRoute, mapRoute } from "./pathview.ts";

const conclusion = (category = "structural"): Conclusion => ({
  authorizes: "", assumptions: [], weakest_evidence: "extracted", is_hypothesis: false, kind: "dependency",
  strength: 2, mathematical: true, complete: true, category, missing_premises: [], caveats: [],
});
const stop = (from: string, to: string) => ({ from, to, from_label: from, to_label: to });
const level = (abstained: boolean): ConceptLevel => ({
  from: { requested: "wd:A", concept: "wd:A", label: "A", via: "concept", derived: false },
  to: { requested: "wd:B", concept: "wd:B", label: "B", via: "concept", derived: false },
  abstained, abstention: abstained ? "no_route" : null, reason: null,
  route: abstained ? null : { score: 1, conclusion: conclusion(), steps: [{
    ...stop("wd:A", "wd:B"), from_derived: false, to_derived: false, relation: "depends_on", direction: "forward",
    evidence_state: "extracted", authorizes: "", route_so_far: "", declaration_claims: 3, counts: [],
    witness: { claim_id: "c1", ...stop("m:a", "m:b"), evidence_state: "extracted", other_premises: 0, sources: [] },
    drill_down: null,
  }] },
});
const response = (over: Partial<PathResponse>): PathResponse => ({
  from: "wd:A", to: "wd:B", objective: "best_supported", weights: {}, abstained: true, reason: null, abstention: "no_path",
  paths: [], exploration: null, composition_version: 3, stats: { algorithm: "", expanded: 0, micros: 0, truncated: false }, ...over,
});

test("the concept route is shown only when the concept level answered", () => {
  assert.equal(conceptRoute(response({})), null, "older server: no concepts field");
  assert.equal(conceptRoute(response({ concepts: null })), null);
  assert.equal(conceptRoute(response({ concepts: level(true) })), null);
  assert.equal(conceptRoute(response({ concepts: level(false) }))?.steps[0].witness.claim_id, "c1");
});

test("the map draws the declaration route first, then the concept route, then the dashed exploration", () => {
  const path = { score: 1, cost_terms: {}, conclusion: conclusion(), steps: [] as never[] };
  const withPath = response({ abstained: false, paths: [{ ...path, steps: [stop("x", "y")] as never[] }], concepts: level(false) });
  assert.deepEqual(mapRoute(withPath).steps.map((s) => s.to), ["y"]);
  const conceptOnly = mapRoute(response({ concepts: level(false) }));
  assert.deepEqual([conceptOnly.steps.map((s) => s.to), conceptOnly.dashed], [["wd:B"], false]);
  const historical = { ...level(false), route: { ...level(false).route!, conclusion: conclusion("historical") } };
  assert.equal(mapRoute(response({ concepts: historical })).dashed, true, "a historical concept route is dashed");
  const explore = mapRoute(response({ concepts: level(true), exploration: { ...path, steps: [stop("p", "q")] as never[] } }));
  assert.deepEqual([explore.steps.map((s) => s.to), explore.dashed], [["q"], true]);
  assert.deepEqual(mapRoute(response({})).steps, [], "nothing to draw: the map is cleared");
});

test("the concept-route strings exist in both languages", () => {
  const es = new Set(uiKeys("es")), en = new Set(uiKeys("en"));
  for (const k of ["conceptRoute", "declarationClaims", "witness", "drillDown", "drillUp", "drillNote", "noDrill"]) assert.ok(es.has(k) && en.has(k), k);
});
