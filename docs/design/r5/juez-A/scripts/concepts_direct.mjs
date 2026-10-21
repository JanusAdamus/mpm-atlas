// Juez A: open the 7 audit concepts by id (search index is not the snapshot's), read the five sections.
import { writeFileSync } from "node:fs";
import { launch, open, go, text, jargon, shot, OUT, pause } from "./lib.mjs";
const [lang = "es"] = process.argv.slice(2);
const IDS = [["integral", "wd:Q80091"], ["derivada", "wd:Q29175"], ["teorema de Pitágoras", "wd:Q11518"], ["teorema de Bayes", "wd:Q182505"],
  ["número primo", "wd:Q49008"], ["pi", "wd:Q167"], ["matriz", "wd:Q44337"]];
const b = await launch();
const rep = { lang, terms: {} };
try {
  const tab = await open(b, { w: 1440, h: 900, theme: "light", lang });
  for (const [term, id] of IDS) {
    await go(tab, `e=${encodeURIComponent(id)}`);
    await tab.waitForSelector("#entity-title", { timeout: 15000 }).catch(() => {});
    await pause(1500);
    const side = await text(tab, "aside.side");
    const q5 = await tab.$$eval("section.q5", (ss) => ss.map((s) => ({ key: s.dataset.q, text: s.innerText })));
    await shot(tab, `jA-${lang}-ficha-${id.replace(":", "_")}`);
    rep.terms[term] = { id, title: await tab.$eval("#entity-title", (e) => e.innerText).catch(() => null), q5, jargonSide: jargon(side), side };
    console.log(term, id, "| q5:", q5.map((q) => q.key).join(","));
  }
  rep.errors = tab.errors;
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-concepts-direct-${lang}.json`, JSON.stringify(rep, null, 1));
console.log("errors", rep.errors);
