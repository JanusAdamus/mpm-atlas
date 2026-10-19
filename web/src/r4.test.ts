// Round 4 (docs/design/codex/2026-10-19-auditoria-usuario-comun.md): what a common reader
// sees in the path card, the record and the concept list, and how the index takes focus.
import { test } from "node:test";
import assert from "node:assert/strict";
import { byClass, byTag, findAll, installDom, textOf, type FakeElement } from "./testdom.ts";
import type { ClaimJson, Conclusion, ConceptLevel, EntityResponse, FoundPath, PathResponse, PathStep } from "./api.ts";
import { renderConcept } from "./conceptpanel.ts";
import { setLang, setMode, t, uiKeys } from "./i18n.ts";
import { contentBounds, Camera } from "./map/camera.ts";
import { mergeEntityPage, renderEntity, type PanelActions } from "./panel.ts";
import { hasMissingPremises, localizedEnd, renderPathResult } from "./pathview.ts";
import { fieldLabel } from "./search.ts";
import { focusReturn } from "./textfocus.ts";

installDom();
setLang("es");
// These checks are about the technical detail; the explore mode has its own tests (panels.test.ts).
setMode("technical");

const conclusion = (over: Partial<Conclusion> = {}): Conclusion => ({
  authorizes: "a nivel de concepto (agregado): se usa para establecer", assumptions: [], weakest_evidence: "model_proposed",
  is_hypothesis: false, kind: "logical_support", strength: 5, mathematical: true, complete: true, category: "mathematical",
  missing_premises: [], caveats: ["Ruta a nivel de concepto."], ...over,
});
const step = (over: Partial<PathStep> = {}): PathStep => ({
  claim_id: "c:1", relation: "depends_on_logically", from: "m:a", to: "m:b", from_label: "HasFDerivAt", to_label: "hasFDerivAt_integral",
  direction: "inverse", evidence_state: "extracted", authorizes: "", route_so_far: "", assumptions: [], premises: ["m:p", "m:q"],
  premises_missing: ["m:q"], premises_satisfied: [], via: [], context: [], validity_domain: null, alternatives: [], sources: [], ...over,
});
const drill: FoundPath = { score: 1, cost_terms: {}, conclusion: conclusion({ weakest_evidence: "extracted" }), steps: [step()] };
const level = (): ConceptLevel => ({
  from: { requested: "wd:Q29175", concept: "wd:Q29175", label: "derivada", via: "concept", derived: false },
  to: { requested: "wd:Q80091", concept: "wd:Q80091", label: "integración", via: "concept", derived: false },
  abstained: false, abstention: null, reason: null,
  route: { score: 1, conclusion: conclusion(), steps: [{
    from: "wd:Q29175", to: "wd:Q80091", from_label: "derivada", to_label: "integración", from_derived: false, to_derived: false,
    relation: "depends_on_logically", direction: "inverse", evidence_state: "model_proposed", authorizes: "", route_so_far: "",
    declaration_claims: 12, counts: [],
    witness: { claim_id: "c:1", from: "m:a", to: "m:b", from_label: "HasFDerivAt", to_label: "hasFDerivAt_integral", evidence_state: "extracted", other_premises: 1, sources: [] },
    drill_down: drill,
  }] },
});
const abstainedWithConcepts = (): PathResponse => ({
  from: "wd:Q29175", to: "wd:Q80091", objective: "best_supported", weights: {}, abstained: true, reason: "only_weak", abstention: "only_weak",
  paths: [], exploration: null, composition_version: 3, concepts: level(), stats: { algorithm: "", expanded: 0, micros: 0, truncated: false },
});
const noop = { open() {}, openConcept() {}, showPath() {}, changed() {} };

test("R4-02: with no validated route the concept route is a candidate, and its strength sits in details with what it measures", () => {
  const root = renderPathResult(abstainedWithConcepts(), noop);
  const text = textOf(root);
  assert.ok(text.includes("Todavía no hay una conexión validada entre estas ideas.  Esto no significa que no estén relacionadas."), text);
  assert.ok(!text.includes("Sin ruta autorizada"));
  const card = findAll(root, byClass("concept-route"))[0];
  assert.equal(textOf(findAll(card, byTag("h3"))[0]), "Conexión candidata entre estos conceptos; pendiente de revisión");
  // The no-route notice is its own block, not part of the candidate card.
  const notice = findAll(root, byClass("no-route"))[0];
  assert.ok(notice && !findAll(notice, byClass("concept-route")).length);
  // Every strength meter of the card is inside a <details>, next to the sentence that says what it measures.
  const meters = findAll(card, byClass("strength"));
  assert.ok(meters.length >= 1);
  const details = findAll(card, byClass("strength-details"));
  assert.equal(details.length, 1);
  assert.equal(findAll(details[0], byClass("strength")).length, meters.length, "no strength in the summary");
  assert.ok(textOf(details[0]).includes("No mide cuán segura es la conexión"));
});

test("R4-06: the drill-down is named as technical backing and explains the crosses before the code", () => {
  const root = renderPathResult(abstainedWithConcepts(), noop);
  const toggle = findAll(root, byClass("drill-toggle"))[0] as FakeElement;
  assert.equal(textOf(toggle), "Ver respaldo técnico (código matemático)");
  toggle.click();
  const box = findAll(root, byClass("drill"))[0];
  const kids = box.children.map((c) => textOf(c));
  assert.ok(kids[0].startsWith("Este es uno de los resultados de código que respalda la conexión propuesta"), kids[0]);
  assert.equal(kids[1], "Las cruces indican premisas que este recorrido no aporta; no significan que sean falsas.");
  assert.ok(hasMissingPremises(drill));
  assert.ok(!hasMissingPremises({ ...drill, steps: [step({ premises_missing: [] })] }));
});

test("R4-06/07 and H09: reader-facing labels in Spanish, no English left in the Spanish UI", () => {
  assert.equal(t("viewConcept"), "Ver resultados técnicos agrupados");
  assert.equal(t("signal_name_doc"), "Coincidencia con el nombre o la descripción del código");
  assert.equal(fieldLabel("description"), "descripción");
  assert.equal(fieldLabel("alias"), "también llamado");
  // Every key differs between languages except the ones that are the same word.
  const same: string[] = [];
  for (const k of uiKeys("es")) {
    setLang("es");
    const es = t(k);
    setLang("en");
    if (es === t(k)) same.push(k);
  }
  setLang("es");
  assert.deepEqual(same.sort(), ["error", "fpsLabel"]);
});

test("R4-07: the concept list says it gathers code, and its way out names the explanation it returns to", () => {
  const root = renderConcept({
    concept: { id: "wd:Q80091", label: "integración", type: "concept", derived: false }, description: "operación en cálculo",
    members_total: 0, memberships_total: 0, memberships_by_state: {}, offset: 0, limit: 50, truncated: false, members: [],
    edges_out_total: 0, edges_out: [], edges_in_total: 0, edges_in: [],
  }, { open() {}, openConcept() {}, expand() {}, collapse() {}, page() {}, drill: () => Promise.reject(new Error("unused")) });
  const text = textOf(root);
  assert.ok(text.includes("Esta lista reúne código relacionado; las asignaciones aún no están revisadas."));
  assert.ok(findAll(root, byTag("button")).some((b) => textOf(b) === "Volver a la explicación de integración"));
  assert.ok(!text.includes("Ficha completa"));
});

const claim = (id: string): ClaimJson => ({
  id, relation: "part_of_area", participants: [{ role: "subject", entity: "wd:Q49008" }, { role: "object", entity: "wd:Q12479" }],
  provenance: [{ source_id: "src:wikidata", locator: "Q49008", extractor: "x", extractor_version: "1", license: "CC0-1.0", retrieved_at: "", quote: null }],
  evidence_state: "extracted", assumptions: [], validity_domain: null, alternative_group: null, supersedes: null, superseded_by: null,
  formal: null, confidence: null, historical_date: null, explanation: {}, disagreements: [], labels: { "wd:Q12479": "teoría de números" },
});
const entity = (claims: ClaimJson[], truncated: boolean): EntityResponse => ({
  entity: {
    id: "wd:Q49008", type: "concept", labels: { es: "número primo" }, aliases: [], descriptions: { es: "número natural mayor que 1" },
    statements: {}, formula: null, external_ids: [["P227", "4047263-1"]],
    provenance: [{ source_id: "src:wikidata", locator: "https://www.wikidata.org/wiki/Q49008", extractor: "x", extractor_version: "1", license: "CC0-1.0", retrieved_at: "", quote: null }],
  },
  canonical_id: "wd:Q49008", merged_from: [], coverage: "known", label: "número primo", counts_by_relation: {}, limit: 2, truncated,
  claims, sources: {},
});
const panelActions: PanelActions = { open() {}, showOnMap() {}, focusRegion() {}, pathFrom() {}, pathTo() {}, openConcept() {} };

test("R4-05: a truncated record offers to load more connections, and pages merge without repeats", () => {
  let asked = 0;
  const root = renderEntity(entity([claim("c:1"), claim("c:2")], true), { ...panelActions, loadMore: () => { asked++; } });
  assert.ok(textOf(root).includes("Estas son las primeras 2 conexiones."));
  const btn = findAll(root, byClass("load-more-btn"))[0];
  assert.equal(textOf(btn), "Cargar más conexiones");
  btn.click();
  assert.equal(asked, 1);
  const merged = mergeEntityPage(entity([claim("c:1"), claim("c:2")], true), entity([claim("c:2"), claim("c:3")], false));
  assert.deepEqual(merged.claims.map((c) => c.id), ["c:1", "c:2", "c:3"]);
  assert.equal(merged.truncated, false);
  assert.ok(!textOf(renderEntity(merged, panelActions)).includes("Cargar más"));
});

test("R4 point 6: the id and external identifiers are folded at the end; Wikidata's description is labelled as such", () => {
  const root = renderEntity(entity([], false), panelActions);
  const header = findAll(root, byTag("header"))[0];
  assert.ok(!textOf(header).includes("wd:Q49008"), "no raw id under the title");
  const ids = findAll(root, byClass("identifiers"));
  assert.equal(ids.length, 1);
  assert.equal(ids[0].tagName, "DETAILS");
  assert.ok(!ids[0].hasAttribute("open"));
  assert.ok(textOf(ids[0]).includes("wd:Q49008") && textOf(ids[0]).includes("P227:4047263-1"));
  // Round 4: the source label sits inside «Qué es», above the description it names.
  const labels = findAll(root, byClass("source-label")).map(textOf);
  assert.ok(labels.includes("Descripción breve (Wikidata)"), labels.join(" | "));
  assert.ok(!textOf(root).includes("Intuición"));
});

test("R4 point 5: a path end opened from a URL gets its label in the active language", async () => {
  assert.deepEqual(await localizedEnd({ id: "wd:Q29175", label: "wd:Q29175" }, async () => "derivada"), { id: "wd:Q29175", label: "derivada" });
  assert.deepEqual(await localizedEnd({ id: "wd:Q29175", label: "derivative" }, async () => "derivada"), { id: "wd:Q29175", label: "derivada" });
  const kept = { id: "wd:Q29175", label: "derivada" };
  assert.deepEqual(await localizedEnd(kept, () => Promise.reject(new Error("offline"))), kept);
});

test("R4-03/04: the index takes focus on open and gives it back on close; the pair is renamed", () => {
  assert.equal(t("textView"), "Ver índice de conceptos");
  assert.equal(t("mapView"), "Volver al mapa");
  const log: string[] = [];
  const el = (name: string) => ({ focus: () => log.push(name), isConnected: true });
  const canvas = el("canvas"), heading = el("heading"), fallback = el("fallback");
  let active: { focus(): void } | null = canvas;
  const f = focusReturn(() => active);
  f.open(heading);
  active = heading;
  f.close(fallback);
  assert.deepEqual(log, ["heading", "canvas"]);
  // A previous focus that left the page falls back to the map.
  active = { focus: () => log.push("gone"), isConnected: false } as never;
  f.open(heading);
  f.close(fallback);
  assert.deepEqual(log.slice(2), ["heading", "fallback"]);
});

test("R4 point 7: the home view frames the content, not the empty unit square", () => {
  // Content in a small square in the middle plus one far outlier.
  const n = 201;
  const px = Float32Array.from({ length: n }, (_, i) => (i === n - 1 ? 0.99 : 0.4 + 0.2 * ((i * 37) % 100) / 100));
  const py = Float32Array.from({ length: n }, (_, i) => (i === n - 1 ? 0.01 : 0.45 + 0.1 * ((i * 53) % 100) / 100));
  const b = contentBounds(px, py)!;
  assert.ok(b[0] > 0.35 && b[2] < 0.65 && b[1] > 0.4 && b[3] < 0.6, `outlier ignored: ${b.join(",")}`);
  const cam = new Camera();
  cam.resize(390, 600);
  const whole = cam.homeView().scale;
  cam.home = b;
  const v = cam.homeView();
  const widthOnScreen = (b[2] - b[0]) * v.scale;
  assert.ok(widthOnScreen > 0.9 * 390 && widthOnScreen <= 390, `content fills the width: ${widthOnScreen}`);
  assert.ok(v.scale > 2 * whole);
  assert.equal(contentBounds(new Float32Array(0), new Float32Array(0)), null);
});
