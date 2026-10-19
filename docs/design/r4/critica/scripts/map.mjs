// Map: label count/overlaps per zoom step (keyboard +), tile level fetched, unknown regions drawn,
// key-driven motion (should be instant), mobile framing, search-result flight, Ruta form flight.
import { writeFileSync } from "node:fs";
import { launch, open, go, labels, OUT, pause, motion, now, shot, text } from "./lib.mjs";
const [pass = "1", W = "1440", theme = "light", mode = "explore", reduce = "0"] = process.argv.slice(2);
const w = +W, h = w < 500 ? 844 : 900;
const b = await launch();
const rep = { pass, w, theme, mode, reduce, zoom: [] };
try {
  const tab = await open(b, { w, h, theme, mode: mode === "technical" ? "technical" : null, reduce: reduce === "1" });
  let tileZ = new Set();
  tab.on("request", (r) => { const m = r.url().match(/\/api\/map\/tiles\/(\d+)\//); if (m) tileZ.add(+m[1]); });
  await go(tab);
  await pause(1200);
  // Framing: spread of label boxes vs canvas.
  rep.framing = await tab.evaluate(() => {
    const c = document.querySelector("canvas.map-gl").getBoundingClientRect();
    const f = (window.__last.length ? window.__last : window.__frame).filter((l) => l.t !== "__unknown__");
    const xs = f.map((l) => l.x), ys = f.map((l) => l.y);
    return { canvas: { x: c.x, y: c.y, w: c.width, h: c.height }, labelsSpan: xs.length ? { x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) } : null, scrollH: document.documentElement.scrollHeight };
  });
  await tab.focus("canvas.map-gl");
  for (let step = 0; step <= 9; step++) {
    tileZ = new Set();
    const t0 = await now(tab);
    if (step > 0) await tab.keyboard.press("+");
    await pause(1600);
    const mo = await motion(tab, t0);
    const L = await labels(tab);
    rep.zoom.push({ step, tileZ: [...tileZ], labels: L.n, unknown: L.unknown, overlaps: L.overlaps, motion: mo.episodes, sample: L.boxes.slice(0, 40).map((x) => x.t) });
    if ([0, 3, 6, 9].includes(step)) await shot(tab, `p${pass}-mapa-zoom${step}-${w}-${theme}-${mode}${reduce === "1" ? "-rm" : ""}`);
    console.log("step", step, "tiles z", [...tileZ].join(","), "labels", L.n, "unknown", L.unknown, "overlaps", L.overlaps.length, "motion", JSON.stringify(mo.episodes));
  }
  // Arrow key pan: motion should be instant (one frame step, no tween).
  {
    const t0 = await now(tab);
    await tab.keyboard.press("ArrowRight");
    await pause(800);
    rep.arrow = await motion(tab, t0);
    const t1 = await now(tab);
    await tab.keyboard.press("n");
    await pause(1200);
    rep.nKey = await motion(tab, t1);
    console.log("arrow", JSON.stringify(rep.arrow.episodes), "N", JSON.stringify(rep.nKey.episodes));
  }
  // Search result -> camera flight.
  await go(tab);
  await pause(800);
  await tab.click("header .searchbox input");
  await tab.keyboard.type("matriz");
  await tab.waitForSelector(".results li[role=option]", { visible: true, timeout: 10000 });
  await pause(400);
  let t0 = await now(tab);
  await tab.keyboard.press("Enter");
  await pause(2500);
  rep.searchFlight = await motion(tab, t0);
  console.log("search flight", JSON.stringify(rep.searchFlight.episodes));
  // Ruta form: derivada -> integral.
  await go(tab);
  const [ruta] = await tab.$$("xpath/.//header//button[normalize-space()='Ruta']");
  await ruta.click();
  await pause(600);
  const ins = await tab.$$(".path-form input[type=search]");
  for (const [i, term] of [[0, "derivada"], [1, "integral"]]) {
    await ins[i].click();
    await tab.keyboard.type(term);
    await tab.waitForSelector(".path-form [role=option], .path-form li", { visible: true, timeout: 8000 }).catch(() => {});
    await pause(600);
    await tab.keyboard.press("Enter");
    await pause(400);
  }
  rep.formEnds = await tab.$$eval(".path-form input[type=search]", (xs) => xs.map((x) => x.value));
  t0 = await now(tab);
  const [buscar] = await tab.$$("xpath/.//button[normalize-space()='Buscar ruta']");
  await buscar.click();
  await pause(3000);
  rep.routeFormFlight = await motion(tab, t0);
  rep.routeFormText = (await text(tab, ".path-result")).slice(0, 800);
  await shot(tab, `p${pass}-ruta-formulario-${w}-${theme}${reduce === "1" ? "-rm" : ""}`);
  console.log("form ends", rep.formEnds, "route flight", JSON.stringify(rep.routeFormFlight.episodes));
  // Route by URL.
  await tab.goto("about:blank");
  await tab.goto(`http://127.0.0.1:8765/`, { waitUntil: "networkidle0" });
  await pause(1500);
  t0 = await now(tab);
  await tab.evaluate(() => { location.hash = "path=1&from=wd:Q29175&to=wd:Q80091&obj=best_supported&hops=6"; });
  await pause(3000);
  rep.routeUrlFlight = await motion(tab, t0);
  console.log("route url flight", JSON.stringify(rep.routeUrlFlight.episodes));
  rep.errors = tab.errors;
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-map-${pass}-${w}-${theme}-${mode}-${reduce}.json`, JSON.stringify(rep, null, 1));
console.log("errors", rep.errors);
