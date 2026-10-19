// Integrator: ink of map labels under the focus veil (C4-08), measured from fillText calls.
import { writeFileSync } from "node:fs";
import { launch, open, go, labels, OUT, pause } from "./lib.mjs";
const lum = (h) => {
  const m = h.match(/^#([0-9a-f]{6})$/i); if (!m) return null;
  const c = [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16) / 255).map((x) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
};
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return +((x + 0.05) / (y + 0.05)).toFixed(2); };
const PAPER = { light: "#f4f1e8", dark: "#14130f" };
const b = await launch();
const rep = {};
try {
  for (const theme of ["light", "dark"]) for (const hash of ["e=wd:Q11518", "e=wd:Q80091", "path=1&from=wd:Q29175&to=wd:Q80091"]) {
    const tab = await open(b, { w: 1440, h: 900, theme });
    await go(tab, hash);
    await pause(1500);
    const l = await labels(tab);
    const fills = {};
    for (const x of l.boxes) fills[x.fill] = (fills[x.fill] || 0) + 1;
    const rated = Object.entries(fills).map(([f, n]) => ({ fill: f, n, ratio: lum(f) == null ? null : ratio(f, PAPER[theme]) }));
    rep[`${theme} ${hash}`] = { labels: l.n, fills: rated, min: Math.min(...rated.filter((r) => r.ratio != null).map((r) => r.ratio)), errors: tab.errors };
    await tab.close();
  }
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-veil.json`, JSON.stringify(rep, null, 1));
console.log(JSON.stringify(rep, null, 1));
