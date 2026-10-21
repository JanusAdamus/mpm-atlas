// Critic helpers (round 4).  Reads the site, never changes it.
import { createRequire } from "node:module";
const require = createRequire("C:/MPM/web/package.json");
const puppeteer = require("puppeteer-core");

export const BASE = "http://127.0.0.1:8811";
export const OUT = "C:/MPM/docs/design/r5/juez-A/ronda-2";
export const pause = (ms) => new Promise((r) => setTimeout(r, ms));

export async function launch() {
  return puppeteer.launch({
    executablePath: "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    headless: true,
    args: ["--enable-gpu", "--ignore-gpu-blocklist", "--use-angle=d3d11", "--enable-unsafe-swiftshader"],
  });
}

// Jargon of the plan's rule, written here from the plan text (not from the product's vocab.ts).
export const JARGON = [
  /wd:/, /mathlib:/i, /Mathlib/, /\bderivado\b/i, /docstring/i, /model_proposed/, /testigo/i,
  /afirmaci[oó]n/i, /declaraci[oó]n/i, /\bR7\b/, /\bQ\d+\b/, /\b\d{1,3}(?:[.,]\d{3})+\s*(nodos|declaraciones|afirmaciones|relaciones)/i,
];
export function jargon(text) {
  const hits = [];
  for (const re of JARGON) {
    const g = new RegExp(re.source, re.flags.includes("g") ? re.flags : re.flags + "g");
    for (const m of text.matchAll(g)) {
      const i = m.index ?? 0;
      hits.push({ re: re.source, match: m[0], ctx: text.slice(Math.max(0, i - 40), i + 40).replace(/\n/g, " ⏎ ") });
    }
  }
  return hits;
}

/** Page with console capture, theme, reduced motion and a fillText recorder on the map overlay. */
export async function open(browser, { w = 1440, h = 900, theme = "light", reduce = false, mode = null, lang = null } = {}) {
  const tab = await browser.newPage();
  tab.errors = [];
  tab.on("pageerror", (e) => tab.errors.push(`pageerror: ${e.message}`));
  tab.on("console", (m) => (m.type() === "error" || m.type() === "warn") && tab.errors.push(`console.${m.type()}: ${m.text()}`));
  tab.on("response", (r) => r.status() >= 400 && tab.errors.push(`HTTP ${r.status()} ${r.url()}`));
  await tab.evaluateOnNewDocument((theme, mode, lang) => {
    try {
      if (theme) localStorage.setItem("atlas.theme", theme);
      if (!sessionStorage.getItem("crit")) {
        if (mode) localStorage.setItem("atlas.mode", mode); else localStorage.removeItem("atlas.mode");
        if (lang) localStorage.setItem("atlas.lang", lang); else localStorage.removeItem("atlas.lang");
        sessionStorage.setItem("crit", "1");
      }
    } catch {}
    // Record overlay text per frame (a full clearRect starts a frame).
    window.__frame = []; window.__last = [];
    const P = CanvasRenderingContext2D.prototype;
    const ft = P.fillText, cr = P.clearRect;
    P.clearRect = function (x, y, ww, hh) {
      if (this.canvas.classList?.contains("map-overlay") && x === 0 && y === 0) {
        if (window.__frame.length) {
          window.__last = window.__frame;
          const sig = window.__frame.slice(0, 12).map((l) => `${l.t}@${Math.round(l.x)},${Math.round(l.y)}`).join("|");
          (window.__frames ||= []).push({ at: window.__frameAt, sig });
          if (window.__frames.length > 3000) window.__frames.shift();
        }
        window.__frame = []; window.__frameAt = performance.now();
      }
      return cr.call(this, x, y, ww, hh);
    };
    const fl = P.fill;
    P.fill = function (...a) {
      if (this.canvas.classList?.contains("map-overlay") && this.fillStyle instanceof CanvasPattern) window.__frame.push({ t: "__unknown__", x: 0, y: 0, w: 0 });
      return fl.apply(this, a);
    };
    let raf = 0; const rq = window.requestAnimationFrame.bind(window);
    window.requestAnimationFrame = (cb) => { window.__raf = ++raf; return rq(cb); };
    P.fillText = function (t, x, y, ...rest) {
      if (this.canvas.classList?.contains("map-overlay")) {
        const m = this.measureText(t);
        const tr = this.getTransform();
        window.__frame.push({ t: String(t), x, y, w: m.width, asc: m.actualBoundingBoxAscent, desc: m.actualBoundingBoxDescent, font: this.font, caps: this.fontVariantCaps, ls: this.letterSpacing, fill: String(this.fillStyle), base: this.textBaseline, align: this.textAlign, sx: tr.a });
      }
      return ft.call(this, t, x, y, ...rest);
    };
  }, theme, mode, lang);
  await tab.emulateMediaFeatures([
    { name: "prefers-reduced-motion", value: reduce ? "reduce" : "no-preference" },
    { name: "prefers-color-scheme", value: theme === "dark" ? "dark" : "light" },
  ]);
  await tab.setViewport(w < 500 ? { width: w, height: h, isMobile: true, hasTouch: true } : { width: w, height: h });
  return tab;
}

export async function go(tab, hash = "") {
  await tab.goto(`${BASE}/${hash ? "#" + hash : ""}`, { waitUntil: "networkidle0", timeout: 30000 });
  await pause(900);
  await tab.waitForNetworkIdle({ idleTime: 500, timeout: 20000 }).catch(() => {});
  await pause(400);
}

/** Labels of the last complete overlay frame, with boxes, and pairwise overlaps. */
export async function labels(tab) {
  return tab.evaluate(() => {
    const all = window.__frame.length ? window.__frame : window.__last;
    const unknown = all.filter((l) => l.t === "__unknown__").length;
    const f = all.filter((l) => !/^\d+$/.test(l.t) && l.t !== "__unknown__");
    const boxes = f.map((l) => {
      const x = l.align === "center" ? l.x - l.w / 2 : l.x;
      const top = l.base === "top" ? l.y : l.y - l.asc;
      return { t: l.t, x, y: top, w: l.w, h: l.asc + l.desc + (l.base === "top" ? 0 : 0), font: l.font, caps: l.caps, fill: l.fill };
    });
    const over = [];
    for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) {
      const a = boxes[i], b = boxes[j];
      const ix = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x), iy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);
      if (ix > 0.5 && iy > 0.5) over.push({ a: a.t, b: b.t, ix: +ix.toFixed(1), iy: +iy.toFixed(1) });
    }
    return { n: boxes.length, unknown, boxes, overlaps: over, frameAt: window.__frameAt };
  });
}

/** Movement episodes of the map since `since` (performance.now): runs of frames whose labels moved,
 *  split by >= gap ms without movement. */
export async function motion(tab, since, gap = 120) {
  return tab.evaluate((since, gap) => {
    const fr = (window.__frames || []).filter((f) => f.at >= since);
    const eps = [];
    let prev = null, cur = null;
    for (const f of fr) {
      const moved = prev && f.sig !== prev.sig;
      if (moved) {
        if (cur && f.at - cur.end <= gap) { cur.end = f.at; cur.frames++; }
        else { cur = { start: prev.at, end: f.at, frames: 1 }; eps.push(cur); }
      }
      prev = f;
    }
    return { frames: fr.length, episodes: eps.map((e) => ({ startMs: Math.round(e.start - since), durMs: Math.round(e.end - e.start), frames: e.frames })) };
  }, since, gap);
}
export const now = (tab) => tab.evaluate(() => performance.now());

export const text =(tab, sel = "body") => tab.evaluate((s) => [...document.querySelectorAll(s)].map((e) => e.innerText).join("\n"), sel);
export const overflow = (tab) => tab.evaluate(() => {
  const res = { page: document.documentElement.scrollWidth - window.innerWidth };
  for (const el of document.querySelectorAll("body *")) {
    const cs = getComputedStyle(el);
    if (el.scrollWidth - el.clientWidth > 1 && /auto|scroll|hidden/.test(cs.overflowX) && el.clientWidth > 0 && cs.display !== "inline") res[(el.className || el.tagName).toString().slice(0, 40)] = el.scrollWidth - el.clientWidth;
  }
  return res;
});
export const shot = (tab, name, full = false) => tab.screenshot({ path: `${OUT}/${name}.png`, fullPage: full });
