// Round 4, second pass on the panels: close-ups of the fixed sections (the full-page
// screenshots of r4panels.mjs stop at the top of each record).
//
//   node bench/r4fix2shots.mjs --base http://127.0.0.1:8765 --out ../docs/design/r4/fix2/paneles
import puppeteer from "puppeteer-core";
import { mkdirSync } from "node:fs";

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const base = arg("base", "http://127.0.0.1:8765");
const out = arg("out", "../docs/design/r4/fix2/paneles");
mkdirSync(out, { recursive: true });
const errors = [];
const browser = await puppeteer.launch({ executablePath: arg("browser", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"), headless: true });
const SHOTS = [
  ["pitagoras-conexiones", "e=wd:Q11518", ".q5[data-q=links]"],
  ["pi-conexiones", "e=wd:Q167", ".q5[data-q=links]"],
  ["matriz-conexiones", "e=wd:Q44337", ".q5[data-q=links]"],
  ["primo-que-es", "e=wd:Q49008", ".q5[data-q=what]"],
];
try {
  for (const theme of ["light", "dark"]) {
    for (const [w, hgt] of [[1440, 900], [390, 844]]) {
      const tab = await browser.newPage();
      tab.on("pageerror", (e) => errors.push(`${theme}-${w} pageerror: ${e.message}`));
      tab.on("console", (m) => m.type() === "error" && errors.push(`${theme}-${w} console: ${m.text()}`));
      await tab.evaluateOnNewDocument((th) => { try { localStorage.setItem("atlas.theme", th); localStorage.removeItem("atlas.mode"); } catch {} }, theme);
      await tab.setViewport(w < 500 ? { width: w, height: hgt, isMobile: true, hasTouch: true } : { width: w, height: hgt });
      for (const [name, hash, sel] of SHOTS) {
        await tab.goto("about:blank");
        await tab.goto(`${base}/#${hash}`, { waitUntil: "networkidle0" });
        await tab.waitForSelector(sel, { timeout: 20000 });
        await tab.waitForFunction(() => !document.querySelector(".wiki-extract .muted"), { timeout: 15000 }).catch(() => {});
        await new Promise((r) => setTimeout(r, 400));
        const el = await tab.$(sel);
        await el.screenshot({ path: `${out}/fix2-${theme}-${w}-${name}.png` });
      }
      await tab.close();
    }
  }
} finally {
  await browser.close();
}
console.log(JSON.stringify({ errors }));
process.exit(errors.length ? 1 : 0);
