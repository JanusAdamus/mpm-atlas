// Go to the left edge (x~0.07, y~0.32 in layout units) where the tiles hold unknown regions.
import { writeFileSync } from "node:fs";
import { launch, open, go, labels, OUT, pause, shot } from "./lib.mjs";
const [theme = "light", pass = "1"] = process.argv.slice(2);
const b = await launch();
const log = [];
try {
  const tab = await open(b, { w: 1440, h: 900, theme });
  await go(tab);
  await tab.focus("canvas.map-gl");
  for (let zs = 0; zs < 6; zs++) {
    for (let i = 0; i < 6; i++) { await tab.keyboard.press("ArrowLeft"); await pause(250); }
    await pause(1200);
    let L = await labels(tab);
    log.push({ zs, unknown: L.unknown, n: L.n });
    if (L.unknown) await shot(tab, `p${pass}-desconocida-${theme}-paso${zs}`);
    await tab.keyboard.press("+");
    await pause(1200);
    L = await labels(tab);
    log.push({ zs: zs + 0.5, unknown: L.unknown, n: L.n, sample: L.boxes.slice(0, 8).map((x) => x.t) });
    if (L.unknown) await shot(tab, `p${pass}-desconocida-${theme}-paso${zs}b`);
  }
  log.push({ legend: await tab.evaluate(() => document.querySelector("[class*=legend]")?.innerText) });
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-unknown2-${theme}-${pass}.json`, JSON.stringify(log, null, 1));
console.log(JSON.stringify(log));
