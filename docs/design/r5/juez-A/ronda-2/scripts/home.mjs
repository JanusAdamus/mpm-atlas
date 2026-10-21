// Home in 4 configurations: visible text, jargon, labels (count, overlaps), overflow, screenshot.
import { writeFileSync } from "node:fs";
import { launch, open, go, labels, text, jargon, overflow, shot, OUT, pause } from "./lib.mjs";
const pass = process.argv[2] ?? "1";
const b = await launch();
const rep = {};
try {
  for (const theme of ["light", "dark"]) for (const [w, h] of [[1440, 900], [390, 844]]) {
    const k = `${theme}-${w}`;
    const tab = await open(b, { w, h, theme });
    const t0 = Date.now();
    await go(tab);
    await pause(1500);
    const L = await labels(tab);
    const body = await text(tab);
    rep[k] = { loadMs: Date.now() - t0, labels: L.n, overlaps: L.overlaps, labelTexts: L.boxes.map((x) => x.t), fonts: [...new Set(L.boxes.map((x) => x.font + "|" + x.caps))], jargon: jargon(body), overflow: await overflow(tab), text: body, errors: tab.errors };
    await shot(tab, `p${pass}-inicio-${k}`);
    await tab.close();
  }
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-home-${pass}.json`, JSON.stringify(rep, null, 1));
for (const [k, v] of Object.entries(rep)) console.log(k, "labels", v.labels, "overlaps", v.overlaps.length, "jargon", v.jargon.length, "overflow", JSON.stringify(v.overflow), "errors", v.errors.length);
console.log(rep["light-1440"].text);
