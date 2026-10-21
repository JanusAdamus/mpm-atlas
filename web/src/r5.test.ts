// Round 5 integration (estado checkpoint 9, judge B): every v4 relation has its ES/EN label and
// inverse verb, the card reads the inverse verb from the object's side, aggregated concept edges
// say «candidata», and name-matched `formalized_as` proposals (explore_hidden) stay out of Explore.
import { test } from "node:test";
import assert from "node:assert/strict";
import { byClass, findAll, installDom, textOf } from "./testdom.ts";
import type { ClaimJson, EntityResponse } from "./api.ts";
import { authKeys, plainKeys, relationKeys, setLang, setMode } from "./i18n.ts";
import { renderEntity, type PanelActions } from "./panel.ts";
import { renderConcept } from "./conceptpanel.ts";
import { RELATION_TYPES } from "./vocab.ts";

installDom();
setLang("es");
setMode("explore");

const prov = { source_id: "src:wikidata", locator: "Q1", extractor: "x", extractor_version: "1", license: "CC0-1.0", retrieved_at: "", quote: null };
let seq = 0;
const claim = (relation: string, subject: string, object: string, labels: Record<string, string>, over: Partial<ClaimJson> = {}): ClaimJson => ({
  id: `c:r5-${++seq}`, relation, participants: [{ role: "subject", entity: subject }, { role: "object", entity: object }],
  provenance: [prov], evidence_state: "extracted", assumptions: [], validity_domain: null, alternative_group: null,
  supersedes: null, superseded_by: null, formal: null, confidence: null, historical_date: null, explanation: {}, disagreements: [], labels, ...over,
});
const record = (id: string, label: string, claims: ClaimJson[]): EntityResponse => ({
  entity: { id, type: "concept", labels: { es: label }, aliases: [], descriptions: { es: "x" }, statements: {}, formula: null, external_ids: [], provenance: [prov] },
  canonical_id: id, merged_from: [], coverage: "known", label, counts_by_relation: {}, limit: 200, truncated: false, claims,
  sources: { "src:wikidata": { id: "src:wikidata", title: "Wikidata", license: "CC0-1.0" } },
});
const actions: PanelActions = { open() {}, showOnMap() {}, focusRegion() {}, pathFrom() {}, pathTo() {}, openConcept() {} };
const links = (r: EntityResponse): string => textOf(findAll(renderEntity(r, actions), byClass("q5")).find((q) => q.getAttribute("data-q") === "links")!);

test("every relation of the vocabulary has a label, both verbs and what it authorizes, in ES and EN", () => {
  for (const lang of ["es", "en"] as const) {
    for (const rel of RELATION_TYPES) {
      assert.ok(relationKeys(lang).includes(rel), `${lang} label ${rel}`);
      assert.ok(plainKeys(lang).includes(rel) && plainKeys(lang).includes(`${rel}~`), `${lang} verbs ${rel}`);
      assert.ok(authKeys(lang).includes(rel), `${lang} authorizes ${rel}`);
    }
  }
});

test("instance_of reads with the inverse verb from the object's side; never the raw English", () => {
  const lab = { "wd:Q80091": "integración", "wd:Q1": "integral abeliana" };
  const c = claim("instance_of", "wd:Q1", "wd:Q80091", lab);
  const fromObject = links(record("wd:Q80091", "integración", [c]));
  assert.ok(fromObject.includes("integración tiene como instancia a integral abeliana."), fromObject);
  assert.ok(!fromObject.includes("instance of") && !fromObject.includes("es una instancia de integral abeliana"), fromObject);
  const fromSubject = links(record("wd:Q1", "integral abeliana", [c]));
  assert.ok(fromSubject.includes("integral abeliana es una instancia de integración."), fromSubject);
  setLang("en");
  assert.ok(links(record("wd:Q80091", "integration", [c])).includes("has as an instance"));
  setLang("es");
});

test("said_equivalent_to is said as a possibility in the source's voice, never as an equivalence", () => {
  const lab = { "wd:Q141488": "grafo", "wd:Q2": "grafo finito" };
  const text = links(record("wd:Q141488", "grafo", [claim("said_equivalent_to", "wd:Q141488", "wd:Q2", lab)]));
  assert.ok(text.includes("Según Wikidata, grafo podría ser lo mismo que grafo finito."), text);
  assert.ok(!text.includes("equivale a") && !text.includes("said equivalent"), text);
});

test("explore_hidden formalized_as proposals are out of Explore and back in the technical mode", () => {
  const lab = { "wd:Q3": "grupo", "mathlib:x": "Grupo" };
  const r = record("wd:Q3", "grupo", [claim("formalized_as", "wd:Q3", "lean:Group", { ...lab, "lean:Group": "Group" }, { evidence_state: "model_proposed", explore_hidden: true })]);
  assert.ok(!links(r).includes("Group"), links(r));
  setMode("technical");
  assert.ok(links(r).includes("Group"), links(r));
  setMode("explore");
});

test("an aggregated concept edge is labeled «candidata»", () => {
  setMode("technical");
  const edge = { concept: { id: "wd:Q4", label: "derivada", type: "concept", derived: false }, total: 3,
    stats: [{ relation: "depends_on_logically", evidence_state: "model_proposed", count: 3, example_claim: "c:x" }], candidate: true };
  const root = renderConcept({
    concept: { id: "wd:Q80091", label: "integración", type: "concept", derived: false }, description: null,
    members_total: 0, memberships_total: 0, memberships_by_state: {}, offset: 0, limit: 50, truncated: false, members: [],
    edges_out_total: 1, edges_out: [edge], edges_in_total: 0, edges_in: [],
  }, { open() {}, openConcept() {}, expand() {}, collapse() {}, page() {}, drill: () => Promise.reject(new Error("unused")) });
  const row = findAll(root, byClass("concept-edge"))[0];
  assert.ok(textOf(row).includes("candidata"), textOf(row));
  setMode("explore");
});
