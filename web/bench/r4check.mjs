// Round 4 browser check (docs/design/codex/2026-10-19-auditoria-usuario-comun.md):
// the derivative -> integration route, its technical backing, the concept index by
// keyboard, the record's paging, the concept list and the phone width.  Screenshots go to
// --out with the --tag prefix (antes / despues).
//
//   node bench/r4check.mjs --base http://127.0.0.1:8771 --out ../docs/design/r4-fix --tag despues
//
// Prints one JSON report; exits 1 on a page or console error, an HTTP status >= 400, a
// `p=` in the visible text or horizontal overflow (page or panels) on the phone.
// `--lenient` (for the "before" run) records the checks without failing on them.
import puppeteer from "puppeteer-core";

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const base = arg("base", "http://127.0.0.1:8771");
const out = arg("out", "../docs/design/r4-fix");
const tag = arg("tag", "despues");
const lenient = process.argv.includes("--lenient");
const browserPath = arg("browser", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe");
const ROUTE = "#path=1&from=wd:Q29175&to=wd:Q80091&obj=best_supported&hops=6";
const PRIME = "#e=wd:Q49008";
const PYTH = "#e=wd:Q11518";
const INTEGRAL = "#e=wd:Q80091";
const CONCEPT = "#c=wd:Q80091";

const errors = [];
const fails = [];
const report = { base, tag, desktop: {}, phone: {} };
const browser = await puppeteer.launch({
  executablePath: browserPath,
  headless: true,
  args: ["--enable-gpu", "--ignore-gpu-blocklist", "--use-angle=d3d11", "--enable-unsafe-swiftshader"],
});
const watch = (tab) => {
  tab.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  tab.on("console", (m) => m.type() === "error" && errors.push(`console: ${m.text()}`));
  tab.on("response", (r) => r.status() >= 400 && errors.push(`HTTP ${r.status()} ${r.url()}`));
};
const shot = (tab, name) => tab.screenshot({ path: `${out}/${tag}-${name}.png` });
const pause = (ms) => new Promise((r) => setTimeout(r, ms));
const noP = async (tab, view) => {
  if (/\bp=/.test(await tab.evaluate(() => document.body.innerText))) fails.push(`${view}: "p=" in the visible text`);
};
/** Run one step; a missing element is recorded, not fatal, so the "before" run completes. */
const step = async (rep, name, fn) => {
  try {
    rep[name] = await fn();
  } catch (e) {
    rep[name] = { error: e.message.split("\n")[0] };
  }
};
const go = async (tab, hash) => {
  await tab.goto(`${base}/`, { waitUntil: "networkidle0" });
  await tab.evaluate((h) => { location.hash = h; }, hash);
  await pause(400);
  await tab.waitForNetworkIdle({ idleTime: 400, timeout: 20000 }).catch(() => {});
};
/** Horizontal overflow of the page and of the panels (scroll width beyond client width). */
const overflow = (tab) => tab.evaluate(() => {
  const o = (el) => (el ? el.scrollWidth - el.clientWidth : null);
  return { page: document.documentElement.scrollWidth - window.innerWidth, main: o(document.querySelector("main")), side: o(document.querySelector("aside.side")), textpane: o(document.querySelector(".textpane")) };
});
const checkOverflow = (view, ov) => {
  for (const [k, v] of Object.entries(ov)) if (v !== null && v > 0) fails.push(`${view}: ${v} px of horizontal overflow in ${k}`);
};
const pathFacts = (tab) => tab.evaluate(() => {
  const inputs = [...document.querySelectorAll(".path-form input[type=search]")];
  const card = document.querySelector(".concept-route");
  const meters = card ? [...card.querySelectorAll(".strength")] : [];
  return {
    ends: inputs.map((i) => ({ value: i.value, title: i.title })),
    notice: document.querySelector(".path-result > .notice")?.innerText ?? null,
    card_title: card?.querySelector("h3")?.innerText ?? null,
    strength_in_summary: meters.filter((m) => !m.closest("details")).length,
    strength_in_details: meters.filter((m) => m.closest("details")).length,
    drill_button: document.querySelector(".drill-toggle")?.innerText ?? null,
  };
});

try {
  // ---------------------------------------------------------------- desktop 1440 x 900
  const tab = await browser.newPage();
  watch(tab);
  await tab.setViewport({ width: 1440, height: 900 });
  const d = report.desktop;
  await step(d, "home", async () => {
    await tab.goto(`${base}/`, { waitUntil: "networkidle0" });
    await pause(800);
    await shot(tab, "01-inicio-1440");
    await noP(tab, "home");
    return true;
  });
  await step(d, "route", async () => {
    await go(tab, ROUTE);
    await tab.waitForSelector(".concept-route", { visible: true, timeout: 15000 });
    await pause(1200);
    await shot(tab, "02-ruta-derivada-1440");
    await noP(tab, "route");
    return await pathFacts(tab);
  });
  await step(d, "deep_zoom_items", async () => {
    // What the route's framing shows on the map, by type (point 8): the index lists the visible items.
    await tab.evaluate(() => document.activeElement?.blur());
    await tab.keyboard.press("t");
    await pause(700);
    const byType = await tab.evaluate(() => {
      const sec = [...document.querySelectorAll(".textpane section")].find((s) => /Entidades visibles|Visible entities/.test(s.querySelector("h3")?.textContent ?? ""));
      const counts = {};
      const sample = [];
      for (const li of sec?.querySelectorAll("li") ?? []) {
        const type = li.querySelector(".muted")?.textContent.replace(/^ · /, "") ?? "?";
        counts[type] = (counts[type] ?? 0) + 1;
        if (sample.length < 40) sample.push(`${li.querySelector("button")?.innerText} · ${type}`);
      }
      return { counts, sample };
    });
    await tab.keyboard.press("t");
    await pause(300);
    return byType;
  });
  await step(d, "drill", async () => {
    await tab.click(".drill-toggle");
    await tab.waitForSelector(".drill:not([hidden]) .route .hop", { visible: true, timeout: 5000 });
    await tab.$eval(".drill:not([hidden])", (el) => el.scrollIntoView({ block: "start" }));
    await pause(500);
    await shot(tab, "03-respaldo-tecnico-1440");
    return await tab.$eval(".drill:not([hidden])", (el) => [...el.children].slice(0, 2).map((c) => c.innerText));
  });
  await step(d, "keyboard_index", async () => {
    await go(tab, "");
    await pause(600);
    await tab.focus("canvas.map-gl");
    await tab.keyboard.press("t");
    await pause(500);
    const afterT = await tab.evaluate(() => ({ id: document.activeElement?.id || null, tag: document.activeElement?.tagName, text: document.activeElement?.innerText?.slice(0, 40) }));
    await tab.keyboard.press("Tab");
    const afterTab = await tab.evaluate(() => {
      const a = document.activeElement;
      const first = document.querySelector(".textpane button");
      return { inIndex: !!a?.closest(".textpane"), isFirstLink: a === first, text: a?.innerText?.slice(0, 40) };
    });
    const button = await tab.$eval(".tools button[aria-pressed]", (b) => b.innerText);
    await shot(tab, "04-indice-teclado-1440");
    await tab.keyboard.press("t");
    await pause(300);
    const back = await tab.evaluate(() => ({ canvas: document.activeElement?.classList.contains("map-gl") ?? false, button: document.querySelector(".tools button[aria-pressed]")?.innerText }));
    if (!lenient && afterT.id !== "textpane-title") fails.push(`T: focus on ${afterT.tag}#${afterT.id}, not the index heading`);
    if (!lenient && !afterTab.isFirstLink) fails.push("Tab after T does not reach the first link of the index");
    if (!lenient && !back.canvas) fails.push("closing the index does not return focus to the map");
    return { afterT, afterTab, button, back };
  });
  await step(d, "prime_paging", async () => {
    await go(tab, PRIME);
    await tab.waitForSelector("#entity-title", { timeout: 15000 });
    const before = await tab.evaluate(() => ({ claims: document.querySelectorAll("li.claim").length, note: document.querySelector(".load-more p, .side > article > p.muted:last-of-type")?.innerText ?? null }));
    await tab.$eval("aside.side", (el) => { el.scrollTop = el.scrollHeight; });
    await pause(300);
    await shot(tab, "05-ficha-primo-final-1440");
    let after = null;
    if (await tab.$(".load-more-btn")) {
      await tab.click(".load-more-btn");
      await tab.waitForFunction((n) => document.querySelectorAll("li.claim").length > n, { timeout: 10000 }, before.claims);
      after = await tab.evaluate(() => ({ claims: document.querySelectorAll("li.claim").length, note: document.querySelector(".load-more p")?.innerText ?? null, focus: document.activeElement?.className }));
      await shot(tab, "06-ficha-primo-mas-1440");
    } else if (!lenient) fails.push("no «Cargar más conexiones» on a truncated record");
    return { before, after };
  });
  await step(d, "integral_record", async () => {
    await go(tab, INTEGRAL);
    await tab.waitForSelector("#entity-title", { timeout: 15000 });
    await pause(400);
    await shot(tab, "07-ficha-integracion-1440");
    return await tab.evaluate(() => ({
      header: document.querySelector(".side article header")?.innerText,
      headings: [...document.querySelectorAll(".side h3")].map((x) => x.innerText),
      ids_folded: !!document.querySelector("details.identifiers:not([open])"),
      concept_button: document.querySelector(".concept-link button")?.innerText ?? null,
    }));
  });
  await step(d, "concept_list", async () => {
    await go(tab, CONCEPT);
    await tab.waitForSelector("#entity-title", { timeout: 15000 });
    await pause(400);
    await shot(tab, "08-concepto-integracion-1440");
    return await tab.evaluate(() => ({
      buttons: [...document.querySelectorAll(".side .actions button")].map((b) => b.innerText),
      note: document.querySelector(".concept-note")?.innerText ?? null,
      signals: [...new Set([...document.querySelectorAll(".side .claim-meta .tag")].map((x) => x.innerText))].slice(0, 6),
    }));
  });
  await step(d, "search_bayes", async () => {
    await go(tab, "");
    await tab.click("header .searchbox input");
    await tab.keyboard.type("Bayes");
    await tab.waitForSelector(".results li[role=option]", { visible: true, timeout: 10000 });
    await pause(400);
    await shot(tab, "09-buscar-bayes-1440");
    const text = await tab.$eval(".results", (el) => el.innerText);
    if (!lenient && /DESCRIPTION|docstring/i.test(text)) fails.push("English field names in the Spanish results");
    return { english_field: /DESCRIPTION/.test(text), fields: [...new Set(await tab.$$eval(".results .field", (els) => els.map((e) => e.innerText)))] };
  });

  // ---------------------------------------------------------------- phone 390 x 844
  const phone = await browser.newPage();
  watch(phone);
  await phone.setViewport({ width: 390, height: 844, isMobile: true, hasTouch: true });
  const p = report.phone;
  await step(p, "home", async () => {
    await phone.goto(`${base}/`, { waitUntil: "networkidle0" });
    await pause(900);
    await shot(phone, "10-inicio-390");
    const ov = await overflow(phone);
    checkOverflow("phone home", ov);
    return ov;
  });
  await step(p, "route_text", async () => {
    await go(phone, ROUTE);
    await phone.waitForSelector(".concept-route", { visible: true, timeout: 15000 });
    await pause(600);
    const facts = await pathFacts(phone);
    await shot(phone, "11-ruta-390");
    await phone.$eval(".tools button[aria-pressed]", (b) => b.click());
    await pause(600);
    const index = await phone.evaluate(() => {
      const pane = document.querySelector(".textpane");
      const r = pane?.getBoundingClientRect();
      return { top: r ? Math.round(r.top) : null, visible: !!r && r.top < window.innerHeight && r.bottom > 0, focus: document.activeElement?.id || document.activeElement?.tagName };
    });
    await shot(phone, "12-indice-con-ruta-390");
    if (!lenient && !index.visible) fails.push(`phone: the index is not visible after opening it (top ${index.top})`);
    if (!lenient && index.focus !== "textpane-title") fails.push(`phone: focus on ${index.focus} after opening the index`);
    const ov = await overflow(phone);
    checkOverflow("phone route + index", ov);
    await noP(phone, "phone route");
    return { facts, index, overflow: ov };
  });
  await step(p, "drill", async () => {
    await go(phone, ROUTE);
    await phone.waitForSelector(".drill-toggle", { visible: true, timeout: 15000 });
    await phone.$eval(".drill-toggle", (b) => b.click());
    await phone.waitForSelector(".drill:not([hidden]) .route .hop", { visible: true, timeout: 5000 });
    await phone.$eval(".drill:not([hidden])", (el) => el.scrollIntoView({ block: "start" }));
    await pause(400);
    await shot(phone, "13-respaldo-tecnico-390");
    const ov = await overflow(phone);
    checkOverflow("phone drill-down", ov);
    return ov;
  });
  await step(p, "pythagoras", async () => {
    await go(phone, PYTH);
    await phone.waitForSelector("#entity-title", { timeout: 15000 });
    await pause(500);
    await phone.$eval("aside.side", (el) => el.scrollIntoView({ block: "start" }));
    await shot(phone, "14-ficha-pitagoras-390");
    const ov = await overflow(phone);
    checkOverflow("phone Pythagoras record", ov);
    const wide = await phone.evaluate(() => [...document.querySelectorAll("aside.side *")]
      .filter((el) => el.getBoundingClientRect().right > window.innerWidth + 0.5)
      .slice(0, 5).map((el) => `${el.tagName}.${el.className} ${Math.round(el.getBoundingClientRect().right)}`));
    return { overflow: ov, wide };
  });
  await step(p, "concept_list", async () => {
    await go(phone, CONCEPT);
    await phone.waitForSelector("#entity-title", { timeout: 15000 });
    await pause(400);
    const ov = await overflow(phone);
    checkOverflow("phone concept list", ov);
    return ov;
  });
} catch (e) {
  errors.push(`r4check: ${e.message}`);
} finally {
  await browser.close();
}
report.errors = errors;
report.fails = fails;
console.log(JSON.stringify(report, null, 1));
process.exit(errors.length || (!lenient && fails.length) ? 1 : 0);
