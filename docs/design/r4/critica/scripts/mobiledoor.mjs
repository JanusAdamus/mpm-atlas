// Phone: open a door, then look for the drawn route on the map (scroll the map into view).
import { launch, open, go, pause, shot } from "./lib.mjs";
const [pass = "1", door = "door_pythagoras"] = process.argv.slice(2);
const b = await launch();
try {
  const tab = await open(b, { w: 390, h: 844 });
  await go(tab);
  await tab.click(`.door[data-door=${door}]`);
  await tab.waitForSelector(".step-card", { visible: true, timeout: 15000 });
  await pause(1500);
  const before = await tab.evaluate(() => { const r = document.querySelector("canvas.map-gl").getBoundingClientRect(); return { y: Math.round(r.y), bottom: Math.round(r.bottom), visible: r.bottom > 0 && r.top < innerHeight }; });
  await tab.evaluate(() => document.querySelector("canvas.map-gl").scrollIntoView({ block: "center" }));
  await pause(1000);
  const stops = await tab.evaluate(() => (window.__frame.length ? window.__frame : window.__last).filter((l) => /^\d+$/.test(l.t)).map((l) => ({ n: l.t, x: Math.round(l.x), y: Math.round(l.y) })));
  const canvas = await tab.evaluate(() => { const r = document.querySelector("canvas.map-gl").getBoundingClientRect(); return { y: Math.round(r.y), w: Math.round(r.width), h: Math.round(r.height) }; });
  await shot(tab, `p${pass}-movil-ruta-mapa-${door}`);
  console.log(JSON.stringify({ door, before, canvas, stops }));
} finally { await b.close(); }
