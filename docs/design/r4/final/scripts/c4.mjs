// Integrator's second pass on the critic's findings C4-01..C4-19 that the critic's scripts do not
// measure directly: route strokes per door, ficha texts (Pythagoras, pi, prime, duplicates),
// phone geometry (toolbar, map, touch targets), legend glyphs.  Reads the site, never changes it.
import { writeFileSync } from "node:fs";
import { launch, open, go, text, OUT, pause, shot, overflow } from "./lib.mjs";
const [theme = "light"] = process.argv.slice(2);
const b = await launch();
const rep = { theme, doors: {}, fichas: {}, phone: {}, legend: null };
const hookStrokes = (tab) => tab.evaluateOnNewDocument(() => {
  const P = CanvasRenderingContext2D.prototype, st = P.stroke;
  P.stroke = function (...a) {
    if (this.canvas.classList?.contains("map-overlay")) (window.__strokes ||= []).push({ dash: this.getLineDash().map((d) => +d.toFixed(1)), style: String(this.strokeStyle), w: this.lineWidth });
    return st.apply(this, a);
  };
});
try {
  // C4-01 / C4-02 / C4-05 / C4-10: strokes and texts of each door at 1440.
  for (const door of ["door_calculus", "door_primes", "door_pythagoras"]) {
    const tab = await open(b, { w: 1440, h: 900, theme });
    await hookStrokes(tab);
    await go(tab);
    const doorText = await tab.$eval(`.door[data-door=${door}]`, (e) => e.innerText);
    await tab.evaluate(() => { window.__strokes = []; });
    await tab.click(`.door[data-door=${door}]`);
    await tab.waitForSelector(".step-card", { visible: true, timeout: 15000 });
    await pause(2500);
    const strokes = await tab.evaluate(() => {
      const seen = new Map();
      for (const s of window.__strokes || []) if (s.w >= 2 && !/rgba\(/.test(s.style)) seen.set(JSON.stringify(s), s);
      return [...seen.values()];
    });
    const side = await text(tab, "aside.side");
    const mapLabels = await tab.evaluate(() => [...new Set((window.__last || []).map((l) => l.t))].filter((t) => !/^\d+$/.test(t)));
    rep.doors[door] = { doorText, strokes, routeReady: (side.match(/Ruta lista[^\n]*/) || [null])[0], seals: [...side.matchAll(/(Documentada|Candidata|Validada|Histórica|Camino por documentos)[^\n]{0,60}/g)].map((m) => m[0]).slice(0, 8), mapLabels: mapLabels.slice(0, 60), errors: tab.errors };
    await shot(tab, `c4-${door}-1440-${theme}`);
    await tab.close();
  }
  // C4-03 / C4-04 / C4-06 / C4-07 / C4-17: ficha texts.
  for (const [name, id] of [["pitagoras", "wd:Q11518"], ["pi", "wd:Q167"], ["primo", "wd:Q49008"], ["derivada", "wd:Q29175"], ["matriz", "wd:Q44337"], ["integral", "wd:Q80091"]]) {
    const tab = await open(b, { w: 1440, h: 900, theme });
    await go(tab, `e=${id}`);
    await tab.waitForSelector("section.q5", { timeout: 15000 }).catch(() => {});
    await pause(1200);
    const q = await tab.$$eval("section.q5", (ss) => Object.fromEntries(ss.map((s) => [s.dataset.q, s.innerText])));
    const linkRows = await tab.$$eval("section.q5[data-q=links] li", (ls) => ls.map((l) => l.innerText.replace(/\s+/g, " ").trim()));
    const counts = {};
    for (const r of linkRows) counts[r] = (counts[r] || 0) + 1;
    rep.fichas[name] = { what: q.what?.slice(0, 700), linksHead: q.links?.slice(0, 300), dupRows: Object.entries(counts).filter(([, n]) => n > 1), nLinkRows: linkRows.length, quinto: (q.links?.match(/quinto postulado[^\n]*/g) || []), ludolph: (q.links?.match(/[^\n]*Ludolph[^\n]*/g) || []), established: /Conexiones establecidas|Established connections/.test(q.links || ""), errors: tab.errors };
    await tab.$eval("section.q5[data-q=links]", (e) => e.scrollIntoView()).catch(() => {});
    await pause(300);
    await shot(tab, `c4-ficha-${name}-1440-${theme}`);
    await tab.close();
  }
  // C4-11 / C4-12 / C4-18: phone geometry.
  {
    const tab = await open(b, { w: 390, h: 844, theme });
    await go(tab);
    await pause(800);
    rep.phone = await tab.evaluate(() => {
      const r = (s) => { const e = document.querySelector(s); if (!e) return null; const b = e.getBoundingClientRect(); return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height), right: Math.round(b.right) }; };
      const small = [...document.querySelectorAll("header button, header a, .tools button, .tools select, .door, .textpane button")]
        .filter((e) => e.offsetParent !== null).map((e) => ({ t: (e.innerText || e.getAttribute("aria-label") || "").trim().slice(0, 30), h: Math.round(e.getBoundingClientRect().height), w: Math.round(e.getBoundingClientRect().width) }))
        .filter((x) => x.h < 44 || x.w < 44);
      const legend = document.querySelector(".map-legend");
      return { tools: r(".tools"), canvas: r("canvas.map-gl"), legendOpen: legend?.open ?? null, legend: r(".map-legend"), smallTargets: small, innerW: innerWidth, innerH: innerHeight };
    });
    rep.phone.overflow = await overflow(tab);
    await shot(tab, `c4-movil-inicio-${theme}`);
    await tab.close();
  }
  // C4-16: legend text (glyphs) at 1440.
  {
    const tab = await open(b, { w: 1440, h: 900, theme });
    await go(tab);
    rep.legend = await tab.$eval(".map-legend", (e) => e.innerText).catch(() => null);
    const el = await tab.$(".map-legend");
    if (el) await el.screenshot({ path: `${OUT}/c4-leyenda-${theme}.png` });
    await tab.close();
  }
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-c4-${theme}.json`, JSON.stringify(rep, null, 1));
console.log(JSON.stringify({ doors: Object.fromEntries(Object.entries(rep.doors).map(([k, v]) => [k, { strokes: v.strokes, routeReady: v.routeReady, doorText: v.doorText }])), phone: rep.phone, legend: rep.legend }, null, 1));
