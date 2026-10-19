// Phone entry: where the map sits, what scrolls, and the map once scrolled into view.
import { writeFileSync } from "node:fs";
import { launch, open, go, OUT, pause, shot, labels } from "./lib.mjs";
const [pass = "1", theme = "light"] = process.argv.slice(2);
const b = await launch();
const rep = {};
try {
  const tab = await open(b, { w: 390, h: 844, theme });
  await go(tab);
  await pause(1000);
  rep.layout = await tab.evaluate(() => {
    const c = document.querySelector("canvas.map-gl");
    const chain = [];
    for (let e = c; e; e = e.parentElement) { const cs = getComputedStyle(e); if (e.scrollHeight > e.clientHeight + 1 && /auto|scroll/.test(cs.overflowY)) chain.push({ el: (e.className || e.tagName).toString().slice(0, 30), sh: e.scrollHeight, ch: e.clientHeight }); }
    const r = c.getBoundingClientRect();
    return { canvas: { y: Math.round(r.y), h: Math.round(r.height), visibleH: Math.round(Math.max(0, Math.min(innerHeight, r.bottom) - Math.max(0, r.top))) }, scrollers: chain, docScroll: document.scrollingElement.scrollHeight };
  });
  await tab.evaluate(() => document.querySelector("canvas.map-gl").scrollIntoView({ block: "end" }));
  await pause(1200);
  rep.after = await tab.evaluate(() => { const r = document.querySelector("canvas.map-gl").getBoundingClientRect(); return { y: Math.round(r.y), h: Math.round(r.height) }; });
  const L = await labels(tab);
  rep.labels = { n: L.n, overlaps: L.overlaps.length, texts: L.boxes.map((x) => x.t) };
  await shot(tab, `p${pass}-movil-mapa-desplazado-${theme}`);
  console.log(JSON.stringify(rep));
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-mobile-${pass}-${theme}.json`, JSON.stringify(rep, null, 1));
