// Computed-style audit: text contrast of every visible text node against its effective background,
// map label colors, HTML font sizes < 12 px, transition/animation durations, legend glyphs, tokens.
import { writeFileSync } from "node:fs";
import { launch, open, go, OUT, pause } from "./lib.mjs";
const [pass = "1"] = process.argv.slice(2);
const VIEWS = [["inicio", ""], ["ficha-integral", "e=wd:Q80091"], ["ruta-calculo", "path=1&from=wd:Q29175&to=wd:Q80091&obj=best_supported&hops=6"], ["ficha-primo", "e=wd:Q49008"]];
const b = await launch();
const rep = {};
try {
  for (const theme of ["light", "dark"]) for (const reduce of [false, true]) for (const mode of ["explore", "technical"]) {
    if (reduce && mode === "technical") continue;
    for (const [w, h] of [[1440, 900], [390, 844]]) {
      const tab = await open(b, { w, h, theme, reduce, mode });
      for (const [vname, hash] of VIEWS) {
        await go(tab, hash);
        await pause(800);
        const k = `${theme}-${w}-${mode}${reduce ? "-rm" : ""}-${vname}`;
        rep[k] = await tab.evaluate(() => {
          const parse = (s) => { const m = s.match(/rgba?\(([^)]+)\)/); if (!m) return null; const p = m[1].split(/[ ,\/]+/).filter(Boolean).map(Number); return { r: p[0], g: p[1], b: p[2], a: p.length > 3 ? p[3] : 1 }; };
          const lin = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
          const L = (c) => 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
          const ratio = (a, b) => { const x = L(a), y = L(b); return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05); };
          const blend = (top, bot) => ({ r: top.r * top.a + bot.r * (1 - top.a), g: top.g * top.a + bot.g * (1 - top.a), b: top.b * top.a + bot.b * (1 - top.a), a: 1 });
          const bgOf = (el) => {
            const stack = [];
            for (let e = el; e; e = e.parentElement) { const c = parse(getComputedStyle(e).backgroundColor); if (c && c.a > 0) { stack.push(c); if (c.a >= 1) break; } }
            let bg = { r: 255, g: 255, b: 255, a: 1 };
            for (let i = stack.length - 1; i >= 0; i--) bg = blend(stack[i], bg);
            return bg;
          };
          const fails = [], small = [];
          let min = 99, n = 0;
          const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
          const seen = new Set();
          for (let t = walker.nextNode(); t; t = walker.nextNode()) {
            const el = t.parentElement;
            if (!el || seen.has(el) || !t.textContent.trim()) continue;
            seen.add(el);
            const r = el.getBoundingClientRect();
            const cs = getComputedStyle(el);
            if (r.width < 1 || r.height < 1 || cs.visibility === "hidden" || +cs.opacity === 0 || el.closest(".sr-only,[hidden],[aria-hidden=true]")) continue;
            if (r.bottom < 0 || r.top > innerHeight * 3) continue;
            const fg0 = parse(cs.color); if (!fg0) continue;
            const bg = bgOf(el);
            const fg = fg0.a < 1 ? blend(fg0, bg) : fg0;
            let op = 1; for (let e = el; e; e = e.parentElement) op *= +getComputedStyle(e).opacity;
            const fgo = op < 1 ? blend({ ...fg, a: op }, bg) : fg;
            const cr = ratio(fgo, bg);
            n++; min = Math.min(min, cr);
            const px = parseFloat(cs.fontSize);
            const large = px >= 24 || (px >= 18.66 && +cs.fontWeight >= 700);
            if (cr < (large ? 3 : 4.5)) fails.push({ text: t.textContent.trim().slice(0, 40), cls: el.className?.toString().slice(0, 40), ratio: +cr.toFixed(2), color: cs.color, bg: `rgb(${bg.r|0},${bg.g|0},${bg.b|0})`, px, opacity: +op.toFixed(2) });
            if (px < 12) small.push({ text: t.textContent.trim().slice(0, 30), cls: el.className?.toString().slice(0, 40), px });
          }
          // Durations.
          const durs = [];
          const ms = (s) => s.split(",").map((x) => (x.trim().endsWith("ms") ? parseFloat(x) : parseFloat(x) * 1000));
          for (const el of document.querySelectorAll("*")) {
            const cs = getComputedStyle(el);
            const td = Math.max(...ms(cs.transitionDuration)), ad = cs.animationName !== "none" ? Math.max(...ms(cs.animationDuration)) : 0;
            if (td > 300 || ad > 300) durs.push({ cls: el.className?.toString().slice(0, 40), td, ad, prop: cs.transitionProperty.slice(0, 40), anim: cs.animationName });
          }
          const root = getComputedStyle(document.documentElement);
          const tok = {};
          for (const name of ["--color-bg-page", "--color-bg-surface", "--color-text-primary", "--color-text-secondary", "--color-text-muted", "--color-accent-solid", "--color-accent-text", "--color-candidate", "--color-danger", "--color-unknown", "--color-border-strong", "--color-border"]) tok[name] = root.getPropertyValue(name).trim();
          const legend = [...document.querySelectorAll(".legend li, .legend [class*=item], .legend span")].map((x) => x.innerText).filter(Boolean).slice(0, 30);
          const hist = [...document.body.innerText].filter((c) => c.codePointAt(0) > 0x2000 && c.codePointAt(0) < 0x2c00).map((c) => "U+" + c.codePointAt(0).toString(16)).filter((v, i, a) => a.indexOf(v) === i);
          return { n, min: +min.toFixed(2), fails: fails.slice(0, 40), nFails: fails.length, small: small.slice(0, 20), nSmall: small.length, durs: durs.slice(0, 20), tok, legend, glyphs: hist, bodyBg: getComputedStyle(document.body).backgroundColor };
        });
        // Map label colors vs map background (canvas bg = body or main bg).
        rep[k].mapLabels = await tab.evaluate(() => {
          const f = (window.__frame.length ? window.__frame : window.__last) || [];
          const fills = {};
          for (const l of f) fills[l.fill] = (fills[l.fill] || 0) + 1;
          return fills;
        });
        console.log(k, "texts", rep[k].n, "min", rep[k].min, "fails", rep[k].nFails, "small", rep[k].nSmall, "durs>300", rep[k].durs.length);
      }
      await tab.close();
    }
  }
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-contrast-${pass}.json`, JSON.stringify(rep, null, 1));
