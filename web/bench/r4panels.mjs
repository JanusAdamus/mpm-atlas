// Round 4, recorridos y paneles: browser check of the entry, the three doors, the record of
// the 7 audit concepts, the step-by-step route by keyboard only, and the text view, in light
// and dark at 1440x900 and 390x844.  The explore mode must show none of the jargon of
// src/vocab.ts JARGON in the visible text of those views.
//
//   node bench/r4panels.mjs --base http://127.0.0.1:8765 --out ../docs/design/r4/paneles
//
// Prints one JSON report; exits 1 on a page or console error, an HTTP status >= 400, jargon in
// explore, `p=` in the visible text, or horizontal overflow (page or panels).
import puppeteer from "puppeteer-core";
import { mkdirSync } from "node:fs";
import { jargonIn } from "../src/vocab.ts";

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const base = arg("base", "http://127.0.0.1:8765");
const out = arg("out", "../docs/design/r4/paneles");
const browserPath = arg("browser", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe");
mkdirSync(out, { recursive: true });

const CONCEPTS = [
  ["integral", "wd:Q80091"], ["derivada", "wd:Q29175"], ["pitagoras", "wd:Q11518"], ["bayes", "wd:Q182505"],
  ["primo", "wd:Q49008"], ["pi", "wd:Q167"], ["matriz", "wd:Q44337"],
];
const DOORS = ["door_calculus", "door_primes", "door_pythagoras"];
const errors = [];
const fails = [];
const report = {};
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const browser = await puppeteer.launch({
  executablePath: browserPath, headless: true,
  args: ["--enable-gpu", "--ignore-gpu-blocklist", "--use-angle=d3d11", "--enable-unsafe-swiftshader"],
});

async function page(theme, w, hgt) {
  const tab = await browser.newPage();
  const tag = `${theme}-${w}`;
  tab.on("pageerror", (e) => errors.push(`${tag} pageerror: ${e.message}`));
  tab.on("console", (m) => m.type() === "error" && errors.push(`${tag} console: ${m.text()}`));
  tab.on("response", (r) => r.status() >= 400 && errors.push(`${tag} HTTP ${r.status()} ${r.url()}`));
  await tab.evaluateOnNewDocument((th) => { try { localStorage.setItem("atlas.theme", th); if (!sessionStorage.getItem("r4")) { localStorage.removeItem("atlas.mode"); sessionStorage.setItem("r4", "1"); } } catch {} }, theme);
  await tab.setViewport(w < 500 ? { width: w, height: hgt, isMobile: true, hasTouch: true } : { width: w, height: hgt });
  return tab;
}
const idle = (tab) => tab.waitForNetworkIdle({ idleTime: 400, timeout: 20000 }).catch(() => {});
async function go(tab, hash) {
  await tab.goto(`${base}/#${hash}`, { waitUntil: "networkidle0" });
  await pause(500);
  await idle(tab);
}
const visible = (tab, sel = "body") => tab.evaluate((s) => [...document.querySelectorAll(s)].map((e) => e.innerText).join("\n"), sel);
async function check(tab, tag, view, sel = "body") {
  const text = await visible(tab, sel);
  const j = jargonIn(text);
  if (j.length) fails.push(`${tag} ${view}: jargon ${JSON.stringify(j)}`);
  if (/\bp=/.test(text)) fails.push(`${tag} ${view}: "p=" in the visible text`);
  const ov = await tab.evaluate(() => {
    const o = (el) => (el ? el.scrollWidth - el.clientWidth : 0);
    return { page: document.documentElement.scrollWidth - window.innerWidth, side: o(document.querySelector("aside.side")), textpane: o(document.querySelector(".textpane")), main: o(document.querySelector("main")) };
  });
  for (const [k, v] of Object.entries(ov)) if (v > 0) fails.push(`${tag} ${view}: ${v} px of horizontal overflow in ${k}`);
  return { jargon: j, overflow: ov };
}
const shot = (tab, tag, name) => tab.screenshot({ path: `${out}/${tag}-${name}.png` });

try {
  for (const theme of ["light", "dark"]) {
    for (const [w, hgt] of [[1440, 900], [390, 844]]) {
      const tag = `${theme}-${w}`;
      const tab = await page(theme, w, hgt);
      const r = (report[tag] = {});
      // Entry.
      await go(tab, "");
      await tab.waitForSelector(".home .ask", { timeout: 15000 });
      await pause(700);
      r.home = await check(tab, tag, "home");
      r.home.question = await tab.$eval(".home .ask", (e) => ({ text: e.innerText, px: getComputedStyle(e).fontSize }));
      r.home.doors = await tab.$$eval(".door", (bs) => bs.map((b) => b.innerText.replace("→", "").trim()));
      r.home.counter = await tab.$eval(".status", (e) => e.innerText).catch(() => "");
      r.home.mode = await tab.$$eval(".mode-switch button", (bs) => bs.map((b) => `${b.innerText}:${b.getAttribute("aria-pressed")}`));
      // C4-11: every toolbar control inside the viewport, the toolbar itself without overflow.
      r.home.tools = await tab.evaluate(() => {
        const tools = document.querySelector(".tools");
        const out = [...tools.querySelectorAll("button, select")].filter((b) => b.offsetParent)
          .map((b) => ({ text: b.innerText.trim(), right: Math.round(b.getBoundingClientRect().right), h: Math.round(b.getBoundingClientRect().height) }));
        return { overflow: tools.scrollWidth - tools.clientWidth, outside: out.filter((b) => b.right > window.innerWidth).map((b) => b.text), controls: out };
      });
      if (r.home.tools.overflow > 0 || r.home.tools.outside.length) fails.push(`${tag} toolbar: ${JSON.stringify(r.home.tools)}`);
      // C4-18: touch targets of 44 px on the phone (inline text links excepted).
      if (w < 500) {
        const small = await tab.evaluate(() => [...document.querySelectorAll("button, select, summary")]
          .filter((b) => b.offsetParent && !b.classList.contains("link") && b.getBoundingClientRect().height < 44)
          .map((b) => `${b.className || b.tagName}:${b.innerText.trim().slice(0, 20)}:${Math.round(b.getBoundingClientRect().height)}`));
        r.home.smallTargets = small;
        if (small.length) fails.push(`${tag} targets under 44 px: ${JSON.stringify(small)}`);
        r.home.legendOpen = await tab.$eval(".map-legend", (d) => d.open);
      }
      // C4-18: the skip link reaches the first door in ≤ 5 Tab (then Enter).
      let skipTabs = 0;
      while (skipTabs < 10 && !(await tab.evaluate(() => !!document.activeElement?.classList.contains("skip") && document.activeElement.textContent.includes("puertas")))) {
        await tab.keyboard.press("Tab");
        skipTabs++;
      }
      await tab.keyboard.press("Enter");
      r.home.skip = { tabs: skipTabs, door: await tab.evaluate(() => document.activeElement?.dataset?.door ?? null) };
      if (skipTabs > 5 || !r.home.skip.door) fails.push(`${tag} skip to doors: ${JSON.stringify(r.home.skip)}`);
      await shot(tab, tag, "01-inicio");
      // Search.
      await tab.click(".home .searchbox input");
      await tab.keyboard.type("integral");
      await tab.waitForSelector(".home .results li[role=option]", { visible: true, timeout: 10000 });
      await pause(300);
      r.search = await check(tab, tag, "search", ".home .results");
      r.search.first = await tab.$eval(".home .results li", (e) => e.innerText.split("\n")[0]);
      await shot(tab, tag, "02-busqueda");
      // Doors.
      r.doors = {};
      for (const d of DOORS) {
        await go(tab, "");
        await tab.waitForSelector(`[data-door=${d}]`, { timeout: 10000 });
        await tab.click(`[data-door=${d}]`);
        await tab.waitForSelector(".stepper .step-card", { visible: true, timeout: 20000 });
        await pause(1200);
        const facts = await tab.evaluate(() => ({
          title: document.querySelector(".stepper h3")?.innerText,
          pos: document.querySelector(".step-pos")?.innerText,
          sentence: document.querySelector(".step-sentence")?.innerText,
          seal: document.querySelector(".step-card .badge")?.innerText,
          focus: document.activeElement?.className,
          stops: [...document.querySelectorAll(".stop-btn")].map((b) => b.innerText.replace(/\n/g, " ")),
          announce: document.querySelector(".pathview > [role=status]")?.textContent,
        }));
        if (/\b1 pasos\b/.test(facts.announce ?? "")) fails.push(`${tag} ${d}: «${facts.announce}»`);
        if (d !== "door_calculus" && !facts.stops.some((x) => x.includes("Artículo de Wikipedia"))) fails.push(`${tag} ${d}: article stops ${JSON.stringify(facts.stops)}`);
        r.doors[d] = { ...facts, ...(await check(tab, tag, `door ${d}`)) };
        if (!facts.title) fails.push(`${tag} ${d}: no route drawn`);
        await shot(tab, tag, `03-puerta-${d.replace("door_", "")}`);
      }
      // Records of the 7 concepts.
      r.records = {};
      for (const [name, id] of CONCEPTS) {
        await go(tab, `e=${id}`);
        await tab.waitForSelector(".q5", { timeout: 15000 });
        await tab.waitForFunction(() => !document.querySelector(".wiki-extract .muted"), { timeout: 10000 }).catch(() => {});
        await pause(300);
        const qs = await tab.$$eval(".q5 > h3", (hs) => hs.map((x) => x.innerText));
        const order = ["QUÉ ES", "POR QUÉ IMPORTA", "QUÉ SABER ANTES", "CON QUÉ SE CONECTA", "CÓMO COMPRUEBO QUE LO ENTENDÍ"];
        if (JSON.stringify(qs.map((q) => q.toUpperCase())) !== JSON.stringify(order)) fails.push(`${tag} ${name}: sections ${JSON.stringify(qs)}`);
        const what = await tab.$eval(".q5[data-q=what]", (e) => e.innerText.slice(0, 400));
        const links = await tab.$eval(".q5[data-q=links]", (e) => e.innerText);
        r.records[name] = { sections: qs.length, what, ...(await check(tab, tag, `record ${name}`)) };
        const want = {
          pitagoras: () => links.includes("dos sentidos") && (links.match(/quinto postulado/g) ?? []).length === 2,
          pi: () => links.includes("Según Wikidata, pi lleva el nombre de Ludolph van Ceulen"),
          primo: () => what.includes("EXTRACTO DE WIKIPEDIA") || what.includes("Extracto de Wikipedia"),
          matriz: () => (links.match(/forma parte de álgebra lineal/g) ?? []).length === 1,
        }[name];
        if (/establecidas/i.test(links) || !/conexiones registradas/i.test(links)) fails.push(`${tag} ${name}: links title`);
        if (want && !want()) fails.push(`${tag} ${name}: fix check failed: ${links.slice(0, 300)} | ${what.slice(0, 200)}`);
        if (theme === "light" || name === "integral") {
          await tab.$eval("aside.side", (e) => { e.scrollTop = 0; e.scrollIntoView({ block: "start" }); });
          await shot(tab, tag, `04-ficha-${name}`);
        }
      }
      // Step-by-step route, keyboard only: Tab to the primes door, Enter, then keys.
      await go(tab, "");
      await tab.waitForSelector("[data-door=door_primes]");
      let tabs = 0;
      while (tabs++ < 60 && !(await tab.evaluate(() => document.activeElement?.dataset?.door === "door_primes"))) await tab.keyboard.press("Tab");
      const reached = await tab.evaluate(() => document.activeElement?.dataset?.door === "door_primes");
      await tab.keyboard.press("Enter");
      await tab.waitForSelector(".stepper .step-card", { visible: true, timeout: 20000 });
      await pause(1000);
      const kb = { reachedDoorWithTab: reached, tabs, focusOnStepper: await tab.evaluate(() => document.activeElement?.classList.contains("stepper")) };
      const pos = () => tab.$eval(".step-pos", (e) => e.innerText);
      kb.start = await pos();
      await tab.keyboard.press("ArrowRight");
      kb.afterRight = await pos();
      await tab.keyboard.press("End");
      kb.afterEnd = await pos();
      await tab.keyboard.press("ArrowLeft");
      kb.afterLeft = await pos();
      // Tab to «Paso siguiente» and press Enter.
      let t2 = 0;
      while (t2++ < 30 && !(await tab.evaluate(() => document.activeElement?.classList.contains("step-next")))) await tab.keyboard.press("Tab");
      await tab.keyboard.press("Enter");
      kb.afterNextButton = await pos();
      await pause(700);
      await shot(tab, tag, "05-ruta-teclado");
      if (!reached || !kb.focusOnStepper || kb.afterRight === kb.start || kb.afterNextButton === kb.afterLeft) fails.push(`${tag} keyboard route: ${JSON.stringify(kb)}`);
      // Leave to a record and come back to the route.
      await tab.click(".step-ends button.link");
      await tab.waitForSelector(".q5", { timeout: 15000 });
      kb.backRoute = !!(await tab.$(".back-route"));
      await tab.click(".back-route");
      await tab.waitForSelector(".stepper .step-card", { visible: true, timeout: 20000 });
      kb.returnedAt = await pos();
      r.keyboard = kb;
      // Text view.
      await tab.evaluate(() => document.activeElement?.blur());
      await tab.keyboard.press("t");
      await pause(900);
      r.text = { focus: await tab.evaluate(() => document.activeElement?.id), ...(await check(tab, tag, "text view", ".textpane")) };
      await shot(tab, tag, "06-indice");
      await tab.keyboard.press("t");
      // Technical mode: the switch shows today's detail and is remembered.
      if (theme === "light") {
        await go(tab, `e=wd:Q167`);
        await tab.waitForSelector(".q5", { timeout: 20000 });
        await tab.click(".mode-switch button:nth-child(2)");
        await pause(800);
        // A fresh load without the mode in the URL: the choice comes from localStorage.
        await tab.goto("about:blank");
        await tab.goto(`${base}/#e=wd:Q167`, { waitUntil: "networkidle0" });
        await tab.waitForSelector(".q5", { timeout: 20000 });
        await pause(400);
        const tech = await tab.evaluate(() => ({ pressed: document.querySelector(".mode-switch button:nth-child(2)")?.getAttribute("aria-pressed"), ids: !!document.querySelector("details.identifiers"), hash: location.hash }));
        r.technical = tech;
        if (tech.pressed !== "true" || !tech.ids) fails.push(`${tag} technical mode not remembered: ${JSON.stringify(tech)}`);
        await shot(tab, tag, "07-ficha-pi-tecnico");
        await go(tab, `e=wd:Q11518`);
        await tab.waitForSelector(".conflict-block", { timeout: 20000 });
        await tab.$eval(".conflict-block", (e) => e.scrollIntoView({ block: "center" }));
        await shot(tab, tag, "08-ficha-pitagoras-tecnico-conflicto");
        await tab.click(".mode-switch button:nth-child(1)");
        await pause(300);
      }
      // C4-09: EN → search → ES: no live region keeps the English count.
      const langBtn = () => tab.evaluateHandle(() => [...document.querySelectorAll(".tools button")].find((b) => /^(EN|ES)$/.test(b.innerText.trim())));
      await tab.goto("about:blank");
      await go(tab, "");
      await tab.waitForSelector(".home .searchbox input");
      await (await langBtn()).click();
      await pause(600);
      await tab.click(".home .searchbox input");
      await tab.keyboard.type("prime");
      await tab.waitForSelector(".home .results li[role=option]", { visible: true, timeout: 10000 });
      await pause(300);
      await (await langBtn()).click();
      await pause(600);
      r.liveRegions = await tab.$$eval("[aria-live]", (els) => els.map((e) => e.textContent).filter(Boolean));
      if (r.liveRegions.some((x) => /\bresults?\b/.test(x))) fails.push(`${tag} live region in English after ES: ${JSON.stringify(r.liveRegions)}`);
      await tab.close();
    }
  }
} catch (e) {
  const where = (e.stack ?? "").split(/\n/).find((l) => l.includes("r4panels.mjs")) ?? "";
  errors.push(`r4panels: ${e.message} @ ${where.trim()}`);
} finally {
  await browser.close();
}
report.errors = errors;
report.fails = fails;
console.log(JSON.stringify(report, null, 1));
process.exit(errors.length || fails.length ? 1 : 0);
