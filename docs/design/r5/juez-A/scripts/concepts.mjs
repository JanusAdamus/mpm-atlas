// Search the 7 audit terms (ES or EN UI), open the record, read the five sections.
import { writeFileSync } from "node:fs";
import { launch, open, go, text, jargon, overflow, shot, OUT, pause } from "./lib.mjs";
const [pass = "1", lang = "es", W = "1440", theme = "light"] = process.argv.slice(2);
const w = +W, h = w < 500 ? 844 : 900;
const TERMS = lang === "es"
  ? [["integral"], ["derivada"], ["teorema de Pitágoras"], ["Bayes", /teorema/i], ["número primo"], ["pi"], ["matriz"]]
  : [["integral"], ["derivative"], ["Pythagorean theorem"], ["Bayes", /theorem/i], ["prime number"], ["pi"], ["matrix"]];
const b = await launch();
const rep = { pass, lang, w, theme, terms: {} };
try {
  const tab = await open(b, { w, h, theme });
  for (const [term, pick] of TERMS) {
    await go(tab);
    if (lang === "en") { const [btn] = await tab.$$("xpath/.//button[normalize-space()='EN']"); if (btn) { await btn.click(); await pause(800); } }
    const input = (await tab.$("header .searchbox input")) ?? (await tab.$("input[type=search]"));
    await input.click();
    const t0 = Date.now();
    await tab.keyboard.type(term);
    await tab.waitForSelector(".results li[role=option]", { visible: true, timeout: 10000 });
    const tFirst = Date.now() - t0;
    await pause(500);
    const results = await tab.$$eval(".results li[role=option]", (ls) => ls.slice(0, 6).map((l) => l.innerText.replace(/\n+/g, " | ")));
    const resultsText = await text(tab, ".results");
    const slug = term.replace(/\s+/g, "-").normalize("NFD").replace(/[^\w-]/g, "");
    await shot(tab, `p${pass}-${lang}-${w}-${theme}-buscar-${slug}`);
    let idx = 0;
    if (pick) idx = Math.max(0, results.findIndex((r) => pick.test(r)));
    const opts = await tab.$$(".results li[role=option]");
    await opts[idx].click();
    await tab.waitForSelector("#entity-title", { timeout: 15000 }).catch(() => {});
    await pause(1200);
    const side = await text(tab, "aside.side");
    const q5 = await tab.$$eval("section.q5", (ss) => ss.map((s) => ({ key: s.dataset.q, text: s.innerText.slice(0, 900) })));
    const hash = await tab.evaluate(() => location.hash);
    await shot(tab, `p${pass}-${lang}-${w}-${theme}-ficha-${slug}`);
    await tab.$eval("aside.side", (el) => { el.scrollTop = el.scrollHeight; }).catch(() => {});
    await pause(300);
    await shot(tab, `p${pass}-${lang}-${w}-${theme}-ficha-${slug}-final`);
    rep.terms[term] = { tFirstResultMs: tFirst, results, picked: results[idx], hash, title: await tab.$eval("#entity-title", (e) => e.innerText).catch(() => null), q5, jargonSide: jargon(side), jargonResults: jargon(resultsText), overflow: await overflow(tab), side };
    console.log(term, tFirst, "ms", "|", results[0], "| q5:", q5.map((q) => q.key).join(","), "| jargon", rep.terms[term].jargonSide.length + rep.terms[term].jargonResults.length);
  }
  rep.errors = tab.errors;
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-concepts-${pass}-${lang}-${w}-${theme}.json`, JSON.stringify(rep, null, 1));
console.log("errors", rep.errors);
