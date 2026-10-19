import { launch, open, go, pause, shot } from "./lib.mjs";
const b = await launch();
try { for (const p of ["1","2"]) {
  const tab = await open(b, {});
  await go(tab, "e=wd:Q49008");
  const n0 = await tab.$$eval("li.claim", (x) => x.length);
  const [btn] = await tab.$$("xpath/.//button[normalize-space()='Cargar más conexiones']");
  await btn.click(); await pause(2000);
  const n1 = await tab.$$eval("li.claim", (x) => x.length);
  const note = await tab.evaluate(() => [...document.querySelectorAll(".side p")].map((p) => p.innerText).filter((t) => /conexiones/.test(t)));
  const groups = await tab.$$eval("section.q5[data-q=links] summary", (s) => s.map((x) => x.innerText.replace(/\n/g, " ")));
  console.log(p, n0, "->", n1, JSON.stringify(note), JSON.stringify(groups));
  await tab.close(); } } finally { await b.close(); }
