import { launch, open, go, pause, shot } from "./lib.mjs";
const b = await launch();
try {
  const tab = await open(b, {});
  await go(tab, "e=wd:Q49008");
  await tab.$$eval("aside.side details", (ds) => ds.forEach((d) => (d.open = true)));
  await pause(500);
  const items = await tab.$$eval("section.q5[data-q=links] li", (ls) => ls.map((l) => l.innerText.replace(/\n+/g, " / ")));
  console.log(items.length, JSON.stringify(items.slice(0, 15)), JSON.stringify(items.filter((x) => /Candidata|espectro|Spectrum/i.test(x)).slice(0, 6)));
  await tab.$eval("section.q5[data-q=links]", (s) => s.scrollIntoView({ block: "start" }));
  await pause(300);
  await shot(tab, "p1-ficha-primo-incluye-desplegado");
} finally { await b.close(); }
