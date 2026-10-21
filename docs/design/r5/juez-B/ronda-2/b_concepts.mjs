// Juez B: common-user test.  Search the 7 audit terms of round 4, open the card, read the five
// sections as shown (drafts folded), then unfold the drafts only to record what they hold.
//   node b_concepts.mjs [pass] [es|en]   -> out-concepts-<pass>-<lang>.json and shots/
import { writeFileSync } from "node:fs";
import { launch, open, go, text, jargon, shot, OUT, pause } from "./b_lib.mjs";
const [pass = "1", lang = "es"] = process.argv.slice(2);
const TERMS = lang === "es"
  ? [["integral"], ["derivada"], ["teorema de Pitágoras"], ["Bayes", /teorema/i], ["número primo"], ["pi"], ["matriz"]]
  : [["integral"], ["derivative"], ["Pythagorean theorem"], ["Bayes", /theorem/i], ["prime number"], ["pi"], ["matrix"]];
const b = await launch();
const rep = { pass, lang, terms: {} };
try {
  const tab = await open(b, { w: 1440, h: 900, theme: "light", lang });
  for (const [term, pick] of TERMS) {
    await go(tab);
    const input = (await tab.$("header .searchbox input")) ?? (await tab.$("input[type=search]"));
    await input.click();
    await tab.keyboard.type(term);
    await tab.waitForSelector(".results li[role=option]", { visible: true, timeout: 10000 });
    await pause(600);
    const results = await tab.$$eval(".results li[role=option]", (ls) => ls.slice(0, 6).map((l) => l.innerText.replace(/\n+/g, " | ")));
    const idx = pick ? Math.max(0, results.findIndex((r) => pick.test(r))) : 0;
    const opts = await tab.$$(".results li[role=option]");
    await opts[idx].click();
    await tab.waitForSelector("#entity-title", { timeout: 15000 }).catch(() => {});
    await pause(2500);
    const slug = term.replace(/\s+/g, "-").normalize("NFD").replace(/[^\w-]/g, "");
    const q5 = await tab.$$eval("section.q5", (ss) => ss.map((s) => ({ key: s.dataset.q, text: s.innerText })));
    await shot(tab, `p${pass}-${lang}-ficha-${slug}`);
    // drafts: unfold every <details> only to record them (not counted as answers)
    const drafts = await tab.$$eval("section.q5", (ss) => ss.map((s) => {
      s.querySelectorAll("details").forEach((d) => { d.open = true; });
      return { key: s.dataset.q, drafts: [...s.querySelectorAll("details.ficha-draft")].map((d) => d.innerText) };
    }));
    const side = await text(tab, "aside.side");
    rep.terms[term] = { results, picked: results[idx], hash: await tab.evaluate(() => location.hash),
      title: await tab.$eval("#entity-title", (e) => e.innerText).catch(() => null), q5, drafts, jargonSide: jargon(side), side };
    console.log(term, "|", results[idx], "| q5:", q5.map((q) => `${q.key}:${q.text.length}`).join(","));
  }
  rep.errors = tab.errors;
} finally { await b.close(); }
writeFileSync(`${OUT}/out-concepts-${pass}-${lang}.json`, JSON.stringify(rep, null, 1));
console.log("errors", rep.errors.length);
