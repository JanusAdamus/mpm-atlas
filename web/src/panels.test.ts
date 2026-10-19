// Round 4, recorridos y paneles (docs/plans/2026-10-19-ronda-4-diseno.md): the explore mode
// shows no jargon in the record, the route and the text view; the record always has the five
// questions in order; the route walks one hop per card with keys; seals come from tokens.ts.
import { test } from "node:test";
import assert from "node:assert/strict";
import { byClass, byTag, findAll, installDom, textOf, type FakeElement } from "./testdom.ts";
import type { ClaimJson, ConceptLevel, EntityResponse, FoundPath, PathResponse, PathStep, SearchHit } from "./api.ts";
import { badge, legendList, SEALS } from "./evidence.ts";
import { authKeys, hopSentence, plainKeys, sealKeys, setLang, setMode, t } from "./i18n.ts";
import type { MapItem } from "./map/map.ts";
import { cleanExtract, prerequisitesOf, renderEntity, type PanelActions } from "./panel.ts";
import { readerLabel, renderPathResult } from "./pathview.ts";
import { readerHits } from "./search.ts";
import { renderTextView } from "./textview.ts";
import { EVIDENCE } from "./tokens.ts";
import { jargonIn } from "./vocab.ts";

installDom();
setLang("es");
setMode("explore");

const prov = (src: string, locator: string) => ({ source_id: src, locator, extractor: "x", extractor_version: "1", license: "CC0-1.0", retrieved_at: "", quote: null });
let seq = 0;
const claim = (relation: string, subject: string, object: string, labels: Record<string, string>, over: Partial<ClaimJson> = {}): ClaimJson => ({
  id: `c:${++seq}`, relation, participants: [{ role: "subject", entity: subject }, { role: "object", entity: object }],
  provenance: [prov("src:wikidata", "Q1")], evidence_state: "extracted", assumptions: [], validity_domain: null, alternative_group: null,
  supersedes: null, superseded_by: null, formal: null, confidence: null, historical_date: null, explanation: {}, disagreements: [], labels, ...over,
});

/** The seven concepts of the audit, with the kinds of claims the registry gives them. */
const AUDIT: [string, string, string, string][] = [
  ["wd:Q80091", "integración", "concept", "operación en cálculo"],
  ["wd:Q29175", "derivada", "area", "operación en cálculo infinitesimal"],
  ["wd:Q11518", "teorema de Pitágoras", "statement", "principio matemático"],
  ["wd:Q182505", "teorema de Bayes", "statement", "teorema que describe la probabilidad de un evento"],
  ["wd:Q49008", "número primo", "area", "número natural mayor que 1 y que solo tiene dos divisores enteros"],
  ["wd:Q167", "pi", "concept", "constante matemática"],
  ["wd:Q44337", "matriz", "area", "conjunto bidimensional de números ordenados en filas y columnas"],
];

function record(id: string, label: string, type: string, desc: string): EntityResponse {
  const claims = [
    claim("appears_in", id, "wp:abc", { "wp:abc": `Wikipedia (es): ${label}` }, { provenance: [prov("src:wikipedia-es", "https://es.wikipedia.org/w/index.php?oldid=1")] }),
    claim("formalized_as", id, "mathlib:Real.pi", { "mathlib:Real.pi": "Real.pi" }, { provenance: [prov("src:mathlib", "Mathlib/Data/Real/Pi.lean")] }),
    claim("part_of_area", "wd:Q7754", id, { "wd:Q7754": "análisis matemático" }),
    claim("special_case_of", id, "wd:Q999", {}, { evidence_state: "model_proposed" }),
    ...(label === "teorema de Bayes" ? [claim("applied_in", id, "wd:Q812535", { "wd:Q812535": "inferencia bayesiana" })] : []),
  ];
  return {
    entity: { id, type, labels: { es: label }, aliases: [], descriptions: { es: desc }, statements: {}, formula: "a^2+b^2=c^2", external_ids: [["P227", "4047263-1"]], provenance: [prov("src:wikidata", `https://www.wikidata.org/wiki/${id.slice(3)}`)] },
    canonical_id: id, merged_from: [], coverage: "known", label, counts_by_relation: { member_of_concept: 876 }, limit: 200, truncated: false, claims,
    sources: { "src:wikidata": { id: "src:wikidata", title: "Wikidata", license: "CC0-1.0", terms: "Structured data under CC0 1.0" }, "src:mathlib": { id: "src:mathlib", title: "Mathlib", license: "Apache-2.0" } },
    concept: { as_concept: { members: 876, memberships: 1636, derived: false }, primary_concept: null },
  };
}
const actions: PanelActions = { open() {}, showOnMap() {}, focusRegion() {}, pathFrom() {}, pathTo() {}, openConcept() {} };
const ORDER = ["Qué es", "Por qué importa", "Qué saber antes", "Con qué se conecta", "Cómo compruebo que lo entendí"];

test("the record of each of the 7 audit concepts has the five questions, in order, with no jargon in explore", () => {
  for (const [id, label, type, desc] of AUDIT) {
    const root = renderEntity(record(id, label, type, desc), actions);
    const qs = findAll(root, byClass("q5"));
    assert.deepEqual(qs.map((q) => textOf(findAll(q, byTag("h3"))[0])), ORDER, label);
    // Each section has content with its source or an honest empty state.
    for (const q of qs) assert.ok(q.children.length >= 2, `${label}: ${textOf(q)}`);
    assert.ok(textOf(qs[0]).includes("Todavía no hay una explicación revisada."));
    assert.ok(textOf(qs[0]).includes("Descripción breve (Wikidata)") && textOf(qs[0]).includes(desc));
    assert.deepEqual(jargonIn(textOf(root)), [], `${label}: ${textOf(root)}`);
    assert.ok(!textOf(root).includes("Real.pi"), "code records stay in the technical mode");
  }
});

test("«Qué saber antes» uses only prerequisite_for and depends_on_logically, and says so", () => {
  const r = record("wd:Q80091", "integración", "concept", "operación en cálculo");
  assert.equal(prerequisitesOf(r).length, 0);
  const before = (rr: EntityResponse) => textOf(findAll(renderEntity(rr, actions), byClass("q5"))[2]);
  assert.ok(before(r).includes("Todavía no hay prerrequisitos registrados") && before(r).includes("«es prerrequisito de»"));
  const withPre = { ...r, claims: [...r.claims,
    claim("prerequisite_for", "wd:Q29175", "wd:Q80091", { "wd:Q29175": "derivada" }),
    claim("depends_on_logically", "wd:Q80091", "wd:Q11348", { "wd:Q11348": "función" }),
    claim("prerequisite_for", "wd:Q80091", "wd:Q5", { "wd:Q5": "ecuación diferencial" })] };
  assert.deepEqual(prerequisitesOf(withPre).map((c) => c.relation), ["prerequisite_for", "depends_on_logically"]);
  assert.ok(before(withPre).includes("derivada") && before(withPre).includes("función") && !before(withPre).includes("ecuación diferencial"));
});

test("technical mode keeps today's detail: ids, provenance and code records", () => {
  setMode("technical");
  const text = textOf(renderEntity(record("wd:Q167", "pi", "concept", "constante matemática"), actions));
  setMode("explore");
  assert.ok(text.includes("wd:Q167") && text.includes("Real.pi") && text.includes("src:wikidata"));
});

test("a Wikipedia extract drops MediaWiki formula leftovers and keeps whole sentences", () => {
  const raw = "Sea A {\\displaystyle A} dado B {\\displaystyle {B}}​ un evento.  Segunda frase.  " + "Tercera frase larga. ".repeat(40);
  const out = cleanExtract(raw, 60);
  assert.ok(out.startsWith("Sea A dado B un evento."), out);
  assert.ok(!out.includes("displaystyle") && !out.includes("​") && out.length <= 60);
});

// ---- route ---------------------------------------------------------------------------

const step = (from: string, to: string, relation: string, over: Partial<PathStep> = {}): PathStep => ({
  claim_id: "c:x", relation, from, to, from_label: from, to_label: to, direction: "forward", evidence_state: "extracted",
  authorizes: "cita bibliografica", route_so_far: "", assumptions: [], premises: [], premises_missing: [], premises_satisfied: [],
  via: [], context: [], validity_domain: null, alternatives: [], sources: [{ source_id: "src:wikipedia-es", locator: "Q5", license: "CC" }], ...over,
});
const conclusion = { authorizes: "", assumptions: [], weakest_evidence: "model_proposed", is_hypothesis: false, kind: "x", strength: 5, mathematical: true, complete: true, category: "documentary", missing_premises: [], caveats: [] };
const exploration: FoundPath = { score: 1, cost_terms: {}, conclusion: { ...conclusion, strength: 0 }, steps: [
  step("número primo", "Wikipedia (es): Número primo", "appears_in"),
  step("Wikipedia (es): Número primo", "Wikipedia (es): Criptografía", "cites", { evidence_state: "model_proposed" }),
  step("Wikipedia (es): Criptografía", "criptografía", "appears_in", { direction: "inverse" }),
] };
const concepts: ConceptLevel = {
  from: { requested: "wd:Q29175", concept: "wd:Q29175", label: "derivada", via: "concept", derived: false },
  to: { requested: "wd:Q80091", concept: "wd:Q80091", label: "integración", via: "concept", derived: false },
  abstained: false, abstention: null, reason: null,
  route: { score: 1, conclusion, steps: [{
    from: "wd:Q29175", to: "wd:Q80091", from_label: "derivada", to_label: "integración", from_derived: false, to_derived: false,
    relation: "depends_on_logically", direction: "inverse", evidence_state: "model_proposed", authorizes: "", route_so_far: "", declaration_claims: 12, counts: [],
    witness: { claim_id: "c:1", from: "mathlib:a", to: "mathlib:b", from_label: "HasFDerivAt", to_label: "hasFDerivAt_integral", evidence_state: "extracted", other_premises: 1, sources: [] },
    drill_down: null,
  }] },
};
const response = (over: Partial<PathResponse>): PathResponse => ({
  from: "wd:Q29175", to: "wd:Q80091", objective: "best_supported", weights: {}, abstained: true,
  reason: "Solo hay una conexion de tipo «alcance bibliografico» (fuerza 0 < 1)", abstention: "only_weak",
  paths: [], exploration, composition_version: 3, concepts: null, stats: { algorithm: "", expanded: 0, micros: 0, truncated: false }, ...over,
});
const pathActions = { open() {}, openConcept() {}, showPath() {}, changed() {} };

test("explore route: no validated route said once, the concept candidate as a plain sentence, no strength, no jargon", () => {
  const root = renderPathResult(response({ concepts }), pathActions);
  const text = textOf(root);
  assert.deepEqual(jargonIn(text), [], text);
  assert.ok(text.includes("Todavía no hay una conexión validada entre estas ideas."));
  assert.ok(!/Fuerza|fuerza/.test(text), "no strength in explore");
  const card = findAll(root, byClass("stepper"))[0];
  assert.equal(textOf(findAll(card, byTag("h3"))[0]), "Conexión candidata entre estos conceptos; pendiente de revisión");
  assert.ok(text.includes("Algunos resultados sobre «integración» usan resultados sobre «derivada»."));
  assert.ok(text.includes("?Candidata") && text.includes("Qué permite concluir"));
});

test("the route walks one hop per card with ←/→, Home/End and the buttons; the map follows", () => {
  const seen: number[] = [];
  const root = renderPathResult(response({}), { ...pathActions, step: (i: number) => seen.push(i) });
  const card = findAll(root, byClass("stepper"))[0];
  const key = (k: string) => card.listeners.get("keydown")![0]({ key: k, target: card, preventDefault() {} } as never);
  const pos = () => textOf(findAll(card, byClass("step-pos"))[0]);
  assert.equal(pos(), "Paso 1 de 3");
  assert.equal(findAll(card, byClass("step-card")).length, 1, "one card");
  key("ArrowRight");
  assert.equal(pos(), "Paso 2 de 3");
  assert.ok(textOf(card).includes("Según Wikipedia, «Artículo de Wikipedia: Número primo» cita a «Artículo de Wikipedia: Criptografía».") && textOf(card).includes("No implica dependencia ni acuerdo"), textOf(card));
  key("End");
  assert.equal(pos(), "Paso 3 de 3");
  assert.ok(textOf(card).includes("«Artículo de Wikipedia: Criptografía» trata de «criptografía».") , "inverse hop read from the side walked");
  (findAll(card, byClass("step-prev"))[0] as FakeElement).click();
  assert.equal(pos(), "Paso 2 de 3");
  key("Home");
  assert.deepEqual(seen, [1, 2, 1, 0]);
  // The stops carry the map's numbers.
  assert.deepEqual(findAll(card, byClass("stop-btn")).map((b) => textOf(findAll(b, byClass("n"))[0])), ["1", "2", "3", "4"]);
});

test("technical route keeps the R4-02 detail: candidate header and the strength inside details", () => {
  setMode("technical");
  const root = renderPathResult(response({ concepts }), pathActions);
  setMode("explore");
  const card = findAll(root, byClass("concept-route"))[0];
  assert.ok(card, "today's concept card is still there");
  const meters = findAll(card, byClass("strength"));
  assert.ok(meters.length >= 1 && findAll(findAll(card, byClass("strength-details"))[0], byClass("strength")).length === meters.length);
});

// ---- seals, text view, search ------------------------------------------------------------

test("seals: glyph and stroke from tokens.ts, plain names, and a legend with every seal once, «Desconocida» included", () => {
  assert.equal(textOf(badge("formally_linked")), `${EVIDENCE.formally_linked.glyph}Validada`);
  assert.equal(textOf(badge("extracted")), "—Documentada");
  assert.equal(textOf(badge("model_proposed")), "?Candidata");
  assert.equal(textOf(badge("disputed")), "!En disputa");
  assert.equal(textOf(badge("retracted")), "×Retirada");
  const names = findAll(legendList(true), byTag("li")).map(textOf);
  assert.equal(names.length, SEALS.length);
  for (const n of ["Validada", "Documentada", "Candidata", "En disputa", "Retirada", "Histórica", "Desconocida"]) assert.ok(names.some((x) => x.includes(n)), n);
  assert.ok(names.find((x) => x.includes("Desconocida"))!.includes("terra incognita"));
  assert.ok(!names.join(" ").includes("Okabe"));
});

const item = (id: string, label: string, kind: MapItem["kind"], size = 12): MapItem => ({ id, label, kind, type: 0, x: 0, y: 0, size, priority: 0, community: "", tile: null as never, index: 0 });

test("text view in explore: ideas only, no code records, ids or internal counts", () => {
  const items = [item("c:1", "Análisis", "cluster", 900), item("wd:Q80091", "integración", "concept", 876), item("mathlib:Real.pi", "Real.pi", "entity"),
    item("wd:Q1", "Q12345", "entity"), item("x:Mathlib", "Mathlib", "concept"), item("wd:Q167", "pi", "entity")];
  const text = textOf(renderTextView(items, new Map([[3, 40]]), { open() {}, openConcept() {}, zoomInto() {}, compare() {} }, 5));
  assert.deepEqual(jargonIn(text), [], text);
  assert.ok(text.includes("integración") && text.includes("pi") && !text.includes("Real.pi") && !text.includes("876"));
  assert.ok(text.includes("Desconocida"));
});

test("search in explore drops code records and labels that are ids", () => {
  const hit = (id: string, label: string): SearchHit => ({ id, type: "concept", label, matched: { field: "label", lang: "es", text: label }, score: 1, lanes: {} });
  const r = readerHits([hit("wd:Q80091", "integración"), hit("mathlib:MeasureTheory.integral_integral", "MeasureTheory.integral_integral"), hit("wd:Q9", "Q9")]);
  assert.deepEqual([r.hits.map((x) => x.id), r.hidden], [["wd:Q80091"], 2]);
});

test("plain verbs, what each hop allows and the seals exist in both languages", () => {
  for (const keys of [plainKeys, authKeys, sealKeys]) assert.deepEqual(keys("es").sort(), keys("en").sort());
  setLang("en");
  assert.equal(hopSentence("depends_on_logically", true, "derivative", "integration"), "Some results about “integration” use results about “derivative”.");
  assert.equal(t("q_check"), "How do I check I understood it");
  setLang("es");
});

test("route stop names: derived-concept tags dropped in explore, kept in technical", () => {
  assert.equal(readerLabel("group › conj (Mathlib, derivado)"), "group › conj");
  assert.equal(readerLabel("Mathlib"), "(sin nombre)");
  assert.equal(readerLabel("derivada"), "derivada");
  setMode("technical");
  assert.equal(readerLabel("group › conj (Mathlib, derivado)"), "group › conj (Mathlib, derivado)");
  setMode("explore");
});
