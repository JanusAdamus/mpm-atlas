// Scan the map for drawn "unknown" regions (hatched pattern fills) at several zooms.
import { writeFileSync } from "node:fs";
import { launch, open, go, labels, OUT, pause, shot } from "./lib.mjs";
const [theme = "light"] = process.argv.slice(2);
const b = await launch();
const found = [];
try {
  const tab = await open(b, { w: 1440, h: 900, theme });
  await go(tab);
  await tab.focus("canvas.map-gl");
  const L0 = await labels(tab);
  found.push({ zoomSteps: 0, pos: "home", unknown: L0.unknown });
  for (const zs of [1, 2, 3, 4, 5, 6, 7, 8]) {
    await tab.keyboard.press("+");
    await pause(1200);
    for (let r = 0; r < 3; r++) {
      for (let c = 0; c < 4; c++) {
        await tab.keyboard.press(r % 2 ? "ArrowLeft" : "ArrowRight");
        await pause(450);
        const L = await labels(tab);
        if (L.unknown) {
          found.push({ zoomSteps: zs, r, c, unknown: L.unknown });
          if (found.filter((f) => f.unknown).length <= 3) await shot(tab, `p1-desconocida-${theme}-z${zs}-${r}${c}`);
        }
      }
      await tab.keyboard.press("ArrowDown");
      await pause(400);
    }
    for (let r = 0; r < 5; r++) await tab.keyboard.press("ArrowUp");
    await pause(500);
  }
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-unknown-deep-${theme}.json`, JSON.stringify(found, null, 1));
console.log(found.length, JSON.stringify(found.slice(0, 20)));
