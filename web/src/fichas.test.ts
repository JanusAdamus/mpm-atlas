// Round 5, point 3 (fichas): the card shows the whole stored Wikipedia lead with the licence of the
// revision it came from; ficha sentences (`explains`) answer a question only once reviewed, the rest
// waits folded under «Borrador editorial, sin revisar», and a sentence is never shown as a connection.
import { test } from "node:test";
import assert from "node:assert/strict";
import { byClass, byTag, findAll, installDom, textOf } from "./testdom.ts";
import type { ClaimJson, EntityResponse } from "./api.ts";
import { setLang, setMode } from "./i18n.ts";
import { cleanExtract, fichaSentences, licenceName, renderEntity, type PanelActions } from "./panel.ts";
import { jargonIn } from "./vocab.ts";

installDom();
setLang("es");
setMode("explore");

const prov = (src: string, locator: string, license = "CC0-1.0") => ({ source_id: src, locator, extractor: "x", extractor_version: "1", license, retrieved_at: "", quote: null });
const WP = "https://es.wikipedia.org/w/index.php?title=N%C3%BAmero_primo&oldid=175494931";
let seq = 0;
const claim = (relation: string, subject: string, object: string, labels: Record<string, string>, over: Partial<ClaimJson> = {}): ClaimJson => ({
  id: `c:${++seq}`, relation, participants: [{ role: "subject", entity: subject }, { role: "object", entity: object }],
  provenance: [prov("src:wikidata", "Q1")], evidence_state: "extracted", assumptions: [], validity_domain: null, alternative_group: null,
  supersedes: null, superseded_by: null, formal: null, confidence: null, historical_date: null, explanation: {}, disagreements: [], labels, ...over,
});
const sentence = (section: string, text: string, state: string): ClaimJson => {
  const note = `note:${section}:${(++seq).toString(16).padStart(16, "0")}`;
  return claim("explains", note, "wd:Q49008", { [note]: text }, { evidence_state: state, provenance: [prov("src:wikipedia-es", `${WP} §entrada`, "CC-BY-SA-4.0")] });
};
const record = (claims: ClaimJson[], descriptions: Record<string, string> = { es: "x" }, provenance = [prov("src:wikidata", "wd:Q49008")]): EntityResponse => ({
  entity: { id: "wd:Q49008", type: "concept", labels: { es: "número primo" }, aliases: [], descriptions, statements: {}, formula: null, external_ids: [], provenance },
  canonical_id: "wd:Q49008", merged_from: [], coverage: "known", label: "número primo", counts_by_relation: {}, limit: 200, truncated: false, claims,
  sources: { "src:wikipedia-es": { id: "src:wikipedia-es", title: "Wikipedia (es)", license: "CC-BY-SA-4.0" } },
});
const actions: PanelActions = { open() {}, showOnMap() {}, focusRegion() {}, pathFrom() {}, pathTo() {}, openConcept() {} };
const q = (root: unknown, key: string) => findAll(root, byClass("q5")).find((s) => s.getAttribute("data-q") === key)!;

test("the card shows the whole stored lead, with the licence and revision of its fragment", async () => {
  const lead = `${"Un número primo es un número natural mayor a 1 que tiene únicamente dos divisores positivos distintos. ".repeat(8)}Después de diez mil es el &&&&&&&&&&010007.&&&&&010 007.`;
  const article = claim("appears_in", "wd:Q49008", "wp:p", { "wp:p": "Wikipedia (es): Número primo" }, { provenance: [prov("src:wikipedia-es", WP, "CC-BY-SA-4.0")] });
  const root = renderEntity(record([article]), { ...actions, extract: async () => record([], { es: lead }, [prov("src:wikipedia-es", WP, "CC-BY-SA-4.0")]) });
  for (let i = 0; i < 3; i++) await new Promise((res) => setTimeout(res, 0));
  const what = textOf(q(root, "what"));
  assert.ok(what.includes("es el 10 007.") && !what.includes("&&"), what);
  assert.ok(what.length > 800, "not cut at 420 characters any more");
  assert.ok(what.includes("CC BY-SA 4.0") && what.includes("texto de la fuente, sin revisar"), what);
  const link = findAll(q(root, "what"), byTag("a")).find((a) => textOf(a).includes("Wikipedia"));
  assert.equal(link?.getAttribute("href"), WP);
  assert.equal(cleanExtract("Corta.", Infinity), "Corta.");
  assert.equal(licenceName("CC-BY-SA-4.0"), "CC BY-SA 4.0");
});

test("ficha sentences: reviewed ones answer, drafts stay folded and labelled, none is a connection", () => {
  const r = record([
    sentence("que_es", "Un número primo es un número natural mayor que 1 cuyos únicos divisores positivos son 1 y él mismo.", "editorially_reviewed"),
    sentence("que_es", "Por convenio, el 1 no es primo ni compuesto.", "model_proposed"),
    sentence("compruebo", "Pregunta: ¿es primo el 7? Respuesta: sí.", "model_proposed"),
  ]);
  assert.equal(fichaSentences(r, "que_es").reviewed.length, 1);
  assert.equal(fichaSentences(r, "compruebo").drafts.length, 1);
  const root = renderEntity(r, actions);
  const what = q(root, "what");
  assert.ok(textOf(findAll(what, byClass("ficha"))[0]).includes("únicos divisores positivos"));
  assert.ok(!textOf(what).includes("Todavía no hay una explicación revisada"), "a reviewed sentence answers the question");
  const [draft] = findAll(what, byClass("ficha-draft"));
  assert.equal(draft.tagName, "DETAILS");
  assert.ok(!draft.getAttribute("open"), "drafts only on request");
  assert.ok(textOf(draft).includes("Borrador editorial, sin revisar") && textOf(draft).includes("el 1 no es primo"));
  const check = textOf(q(root, "check"));
  assert.ok(check.includes("Todavía no hay una explicación revisada") && check.includes("Borrador editorial, sin revisar"), check);
  assert.ok(!textOf(q(root, "links")).includes("el 1 no es primo"), "an explains claim is not a connection");
  assert.deepEqual(jargonIn(textOf(root)), []);
});
