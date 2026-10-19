// Open every collapsed group of the record (explore mode) and scan for jargon.
import { launch, open, go, pause, shot, text, jargon } from "./lib.mjs";
const [pass = "1"] = process.argv.slice(2);
const b = await launch();
try {
  const tab = await open(b, {});
  for (const [n, id] of [["primo", "wd:Q49008"], ["derivada", "wd:Q29175"], ["matriz", "wd:Q44337"], ["integral", "wd:Q80091"], ["pi", "wd:Q167"], ["pitagoras", "wd:Q11518"], ["bayes", "wd:Q182505"]]) {
    await go(tab, `e=${id}`);
    await tab.$$eval("aside.side details", (ds) => ds.forEach((d) => (d.open = true)));
    await pause(500);
    const side = await text(tab, "aside.side");
    const j = jargon(side);
    const hit = await tab.evaluate(() => { const el = [...document.querySelectorAll("aside.side li, aside.side a")].find((e) => /Mathlib/.test(e.innerText)); if (el) { el.scrollIntoView({ block: "center" }); return el.innerText.replace(/\n/g, " / "); } return null; });
    await pause(300);
    if (hit) await shot(tab, `p${pass}-jerga-desplegada-${n}`);
    console.log(n, "jargon", j.length, [...new Set(j.map((x) => x.match))].slice(0, 6), "|", hit);
  }
} finally { await b.close(); }
