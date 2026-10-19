// Round 4, second pass on the panels (docs/design/r4/critica/2026-10-19-critica-r4.md): one
// check per finding, each failing on the code before the fix.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { byClass, byTag, findAll, installDom, textOf } from "./testdom.ts";
import type { ClaimJson, EntityResponse } from "./api.ts";
import { setLang, setMode, t } from "./i18n.ts";
import { renderEntity, type PanelActions } from "./panel.ts";
import { readerLabel } from "./pathview.ts";
import { searchBox } from "./search.ts";
import { renderTextView } from "./textview.ts";
import type { MapItem } from "./map/map.ts";

installDom();
setLang("es");
setMode("explore");

const prov = (src: string, locator: string) => ({ source_id: src, locator, extractor: "x", extractor_version: "1", license: "CC0-1.0", retrieved_at: "", quote: null });
let seq = 0;
const claim = (relation: string, subject: string, object: string, labels: Record<string, string>, over: Partial<ClaimJson> = {}): ClaimJson => ({
  id: `c:${++seq}`, relation, participants: [{ role: "subject", entity: subject }, { role: "object", entity: object }],
  provenance: [prov("src:wikidata", `${subject}#P${seq}`)], evidence_state: "extracted", assumptions: [], validity_domain: null, alternative_group: null,
  supersedes: null, superseded_by: null, formal: null, confidence: null, historical_date: null, explanation: {}, disagreements: [], labels, ...over,
});
const record = (id: string, label: string, claims: ClaimJson[], over: Partial<EntityResponse> = {}): EntityResponse => ({
  entity: { id, type: "concept", labels: { es: label }, aliases: [], descriptions: { es: "x" }, statements: {}, formula: null, external_ids: [], provenance: [prov("src:wikidata", id)] },
  canonical_id: id, merged_from: [], coverage: "known", label, counts_by_relation: {}, limit: 200, truncated: false, claims,
  sources: { "src:wikidata": { id: "src:wikidata", title: "Wikidata", license: "CC0-1.0" } }, ...over,
});
const actions: PanelActions = { open() {}, showOnMap() {}, focusRegion() {}, pathFrom() {}, pathTo() {}, openConcept() {} };
const links = (r: EntityResponse) => findAll(renderEntity(r, actions), byClass("q5")).find((q) => q.dataset.q === "links" || q.getAttribute("data-q") === "links")!;

test("C4-03: the same relation recorded in both directions is one conflict, not two facts; nothing is dropped", () => {
  const pyth = "wd:Q11518", post = "wd:Q1062";
  const lab = { [pyth]: "teorema de Pitágoras", [post]: "quinto postulado de Euclides", "wd:Q7": "teorema del coseno" };
  const r = record(pyth, "teorema de Pitágoras", [
    claim("generalizes", pyth, post, lab), claim("generalizes", post, pyth, lab), claim("generalizes", "wd:Q7", pyth, lab),
  ]);
  for (const mode of ["explore", "technical"] as const) {
    setMode(mode);
    const sec = links(r);
    const block = findAll(sec, byClass("conflict-block"))[0];
    assert.ok(block, mode);
    assert.ok(textOf(block).includes("La fuente registra esta relación en los dos sentidos; está pendiente de revisión."));
    const rows = findAll(block, byClass("conflict"));
    assert.equal(rows.length, 2, `${mode}: both claims kept`);
    for (const row of rows) assert.ok(textOf(row).includes("En conflicto") && textOf(row).includes("quinto postulado"), textOf(row));
    // The postulate is no longer listed as a plain fact outside the block; the cosine law is.
    const outside = findAll(sec, byTag("details")).map(textOf).join(" ");
    assert.ok(!outside.includes("quinto postulado") && outside.includes("teorema del coseno"), outside);
  }
  setMode("explore");
});

test("C4-06: an unreviewed relation is said in its source's voice, with its seal", () => {
  const r = record("wd:Q167", "pi", [claim("named_after", "wd:Q167", "wd:Q4", { "wd:Q4": "Ludolph van Ceulen" })]);
  const row = findAll(links(r), byClass("claim"))[0];
  assert.ok(textOf(row).includes("Según Wikidata, pi lleva el nombre de Ludolph van Ceulen."), textOf(row));
  assert.ok(textOf(findAll(row, byClass("badge"))[0]).includes("Documentada"));
  // Articles read as articles in the record too (C4-05).
  const art = record("wd:Q167", "pi", [claim("appears_in", "wd:Q167", "wp:x", { "wp:x": "Wikipedia (en): Pi" }, { provenance: [prov("src:wikipedia-en", "u")] })]);
  assert.ok(textOf(links(art)).includes("Según Wikipedia, pi se trata en Artículo de Wikipedia (en inglés): Pi."), textOf(links(art)));
  // A reviewed one is not attributed.
  const reviewed = record("wd:Q167", "pi", [claim("named_after", "wd:Q167", "wd:Q4", { "wd:Q4": "Ludolph van Ceulen" }, { evidence_state: "editorially_reviewed" })]);
  assert.ok(!textOf(links(reviewed)).includes("Según"));
});

test("C4-07: «Conexiones registradas», and a line saying none is reviewed", () => {
  const r = record("wd:Q167", "pi", [claim("special_case_of", "wd:Q167", "wd:Q5", { "wd:Q5": "número real" })]);
  const text = textOf(links(r));
  assert.ok(text.includes("Conexiones registradas") && !text.includes("establecidas"), text);
  assert.ok(text.includes("nadie las ha revisado todavía"));
});

test("C4-05: route stops that are Wikipedia articles read as articles, with the title's language when it is not the reader's", () => {
  assert.equal(readerLabel("Wikipedia (es): Número primo"), "Artículo de Wikipedia: Número primo");
  assert.equal(readerLabel("Wikipedia (en): Pythagorean theorem"), "Artículo de Wikipedia (en inglés): Pythagorean theorem");
  setLang("en");
  assert.equal(readerLabel("Wikipedia (en): Pythagorean theorem"), "Wikipedia article: Pythagorean theorem");
  setLang("es");
  assert.equal(readerLabel("número primo"), "número primo");
});

test("plurals: «1 paso», «1 resultado» in ES and EN", () => {
  assert.equal(t("routeReady", { n: 1 }), "Ruta lista: 1 paso.");
  assert.equal(t("routeReady", { n: 3 }), "Ruta lista: 3 pasos.");
  assert.equal(t("results", { n: 1 }), "1 resultado");
  assert.equal(t("results", { n: "1" }), "1 resultado", "numbers passed as text count too");
  setLang("en");
  assert.equal(t("routeReady", { n: 1 }), "Route ready: 1 step.");
  assert.equal(t("results", { n: 5 }), "5 results");
  setLang("es");
});

test("C4-09: changing the language clears the search announcement («5 results» does not stay)", () => {
  setLang("en");
  const box = searchBox({ label: t("search"), placeholder: t("searchPlaceholder"), onPick() {} });
  const status = findAll(box.root, (el) => el.getAttribute("role") === "status")[0];
  status.textContent = t("results", { n: 5 });
  setLang("es");
  box.relabel(t("search"), t("searchPlaceholder"));
  assert.equal(textOf(status), "");
  assert.equal(textOf(findAll(box.root, byTag("label"))[0]), "Buscar");
});

test("C4-17: one row per relation and destination, with how many source records back it", () => {
  const m = "wd:Q44337", al = "wd:Q82571";
  const lab = { [al]: "álgebra lineal" };
  const r = record(m, "matriz", [claim("part_of_area", m, al, lab), claim("part_of_area", m, al, lab), claim("part_of_area", m, al, lab)]);
  const rows = findAll(links(r), byClass("claim"));
  assert.equal(rows.length, 1);
  assert.ok(textOf(rows[0]).includes("3 registros en las fuentes"), textOf(rows[0]));
  setMode("technical");
  assert.equal(findAll(links(r), byClass("claim")).length, 3, "technical keeps every claim");
  setMode("explore");
});

test("C4-04: the Wikipedia extract is found past the first page, and the formula is labelled «Fórmula (Wikidata)»", async () => {
  const id = "wd:Q49008";
  const many = Array.from({ length: 3 }, (_, i) => claim("part_of_area", `wd:N${i}`, id, { [`wd:N${i}`]: String(163 + i) }));
  const article = claim("appears_in", id, "wp:p", { "wp:p": "Wikipedia (es): Número primo" }, { provenance: [prov("src:wikipedia-es", "https://es.wikipedia.org/w/index.php?oldid=1")] });
  const r = record(id, "número primo", many, { truncated: true, counts_by_relation: { appears_in: 1, part_of_area: 545 } });
  const withFormula = { ...r, entity: { ...r.entity, formula: "p: \\tau(p) = 2" } };
  const asked: number[] = [];
  const root = renderEntity(withFormula, {
    ...actions,
    claimsFrom: async (offset) => { asked.push(offset); return { ...r, truncated: false, claims: [...many, article] }; },
    extract: async () => record("wp:p", "Número primo", [], { entity: { ...r.entity, descriptions: { es: "Un número primo es un número natural mayor que 1 que tiene únicamente dos divisores positivos distintos." } } }),
  });
  for (let i = 0; i < 5; i++) await new Promise((res) => setTimeout(res, 0));
  const what = textOf(findAll(root, byClass("q5"))[0]);
  assert.deepEqual(asked, [3]);
  assert.ok(what.includes("Extracto de Wikipedia (es)") && what.includes("dos divisores positivos distintos"), what);
  assert.ok(what.includes("Fórmula (Wikidata)"), what);
});

test("C4-11 / C4-18: on the phone the toolbar wraps (no sideways scroll) and its buttons are 44 px", () => {
  const css = readFileSync(new URL("./style.css", import.meta.url), "utf8");
  const phone = css.slice(css.indexOf("@media (max-width: 760px)"));
  const rule = (sel: string) => new RegExp(`\\n\\s*${sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*\\{([^}]*)\\}`).exec(phone)?.[1] ?? "";
  assert.ok(/flex-wrap:\s*wrap/.test(rule(".tools")) && !/overflow-x:\s*auto/.test(rule(".tools")), rule(".tools"));
  assert.ok(/min-height:\s*var\(--target-touch\)/.test(rule(".tools button, .tools select")));
  assert.ok(/min-height:\s*var\(--target-touch\)/.test(rule(".textview li button")));
});

test("concept index: regions take the map's name, never «Wikipedia (es): …»; the legend explains the points", () => {
  const it = (id: string, label: string): MapItem => ({ id, label, kind: "cluster", type: 0, x: 0, y: 0, size: 9, priority: 0, community: "", tile: null as never, index: 0 });
  const names = new Map([["c:1", "estadística"], ["c:2", null]]);
  const view = renderTextView([it("c:1", "Wikipedia (es): Estadística"), it("c:2", "Wikipedia (en): Temperature")], new Map(), { open() {}, openConcept() {}, zoomInto() {}, compare() {} }, -1, (x) => names.get(x.id) ?? null);
  const text = textOf(view);
  assert.ok(text.includes("estadística") && !text.includes("Wikipedia (") , text);
  assert.ok(text.includes("Los puntos son de un solo color") && text.includes("◎ concepto, ◯ región, ● otra idea"));
});
