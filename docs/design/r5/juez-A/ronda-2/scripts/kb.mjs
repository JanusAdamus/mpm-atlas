// Keyboard index, phone index with route open, text view jargon + render loop, language, technical mode, live regions.
import { writeFileSync } from "node:fs";
import { launch, open, go, OUT, pause, shot, text, jargon, overflow } from "./lib.mjs";
const [pass = "1", theme = "light"] = process.argv.slice(2);
const b = await launch();
const rep = {};
const active = (tab) => tab.evaluate(() => { const a = document.activeElement; return { tag: a?.tagName, id: a?.id || null, cls: a?.className?.toString().slice(0, 40), text: (a?.innerText || a?.getAttribute?.("aria-label") || "").slice(0, 50), inIndex: !!a?.closest?.(".textpane") }; });
const status = (tab) => tab.$$eval("[role=status],[aria-live]", (xs) => xs.map((x) => x.innerText.trim()).filter(Boolean));
try {
  // 1. Desktop keyboard: Tab to canvas, T, Tab.
  {
    const tab = await open(b, { theme });
    await go(tab);
    let n = 0;
    for (; n < 60; n++) { await tab.keyboard.press("Tab"); if (await tab.evaluate(() => document.activeElement?.classList.contains("map-gl"))) break; }
    const r = { tabsToCanvas: n + 1 };
    await tab.keyboard.press("t");
    await pause(600);
    r.afterT = await active(tab);
    await tab.keyboard.press("Tab");
    r.afterTab = await active(tab);
    r.firstIndexItem = await tab.$eval(".textpane button, .textpane a", (x) => x.innerText.slice(0, 40)).catch(() => null);
    await shot(tab, `p${pass}-teclado-indice-1440-${theme}`);
    const tp = await text(tab, ".textpane");
    r.textpaneJargon = jargon(tp);
    r.textpaneHead = tp.slice(0, 700);
    // Render loop while text view is active.
    const raf0 = await tab.evaluate(() => window.__raf || 0); await pause(2000); const raf1 = await tab.evaluate(() => window.__raf || 0);
    r.rafPer2sTextView = raf1 - raf0;
    await tab.keyboard.press("Escape"); await pause(200);
    await tab.evaluate(() => document.activeElement?.blur());
    await tab.keyboard.press("t"); await pause(600);
    r.afterBack = await active(tab);
    const raf2 = await tab.evaluate(() => window.__raf || 0); await pause(2000); const raf3 = await tab.evaluate(() => window.__raf || 0);
    r.rafPer2sMap = raf3 - raf2;
    // Search live region.
    await tab.click("header .searchbox input");
    await tab.keyboard.type("integral");
    await pause(150);
    r.statusTyping = await status(tab);
    await pause(900);
    r.statusAfter = await status(tab);
    r.errors = tab.errors;
    rep.desktop = r;
    console.log("desktop", JSON.stringify({ ...r, textpaneHead: undefined }));
    await tab.close();
  }
  // 2. Phone with route open: "Ver índice de conceptos".
  {
    const tab = await open(b, { w: 390, h: 844, theme });
    await go(tab, "path=1&from=wd:Q29175&to=wd:Q80091&obj=best_supported&hops=6");
    await tab.waitForSelector(".step-card", { timeout: 15000 }).catch(() => {});
    await pause(800);
    const [btn] = await tab.$$("xpath/.//button[contains(normalize-space(),'índice de conceptos')]");
    const btnBox = btn ? await btn.boundingBox() : null;
    if (btn) { await btn.click(); await pause(900); }
    const r = { btnBox };
    r.active = await active(tab);
    r.textpane = await tab.evaluate(() => { const t = document.querySelector(".textpane"); if (!t) return null; const b = t.getBoundingClientRect(); return { top: Math.round(b.top), h: Math.round(b.height), scrollY: scrollY }; });
    r.overflow = await overflow(tab);
    r.tapTargets = await tab.evaluate(() => [...document.querySelectorAll("button, a, input, select, [role=option]")].filter((e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && r.top < innerHeight && r.bottom > 0; }).map((e) => { const r = e.getBoundingClientRect(); return { t: (e.innerText || e.getAttribute("aria-label") || e.tagName).trim().slice(0, 30), w: Math.round(r.width), h: Math.round(r.height) }; }).filter((x) => x.h < 44 || x.w < 44));
    await shot(tab, `p${pass}-movil-indice-con-ruta-${theme}`);
    rep.phoneIndex = r;
    console.log("phone", JSON.stringify(r).slice(0, 1500));
    // Toolbar overflow: which controls are hidden off-screen.
    await go(tab);
    rep.phoneTools = await tab.evaluate(() => { const t = document.querySelector(".tools"); return t ? { scrollW: t.scrollWidth, clientW: t.clientWidth, ox: getComputedStyle(t).overflowX, hidden: [...t.querySelectorAll("button, select, a")].filter((e) => { const r = e.getBoundingClientRect(); return r.right > innerWidth || r.left < 0; }).map((e) => e.innerText.trim() || e.getAttribute("aria-label")) } : null; });
    console.log("phoneTools", JSON.stringify(rep.phoneTools));
    await tab.close();
  }
  // 3. Language switch.
  {
    const tab = await open(b, { theme });
    await go(tab, "e=wd:Q80091");
    const [en] = await tab.$$("xpath/.//button[normalize-space()='EN']");
    await en.click(); await pause(1500);
    const r = {};
    r.hash = await tab.evaluate(() => location.hash);
    r.searchA11y = await tab.$eval("header .searchbox input", (i) => ({ aria: i.getAttribute("aria-label"), ph: i.placeholder, label: i.labels?.[0]?.innerText }));
    const body = await text(tab);
    r.spanishLeft = [...new Set(body.match(/\b(Buscar|Ruta|Validada|Documentada|Candidata|Qué es|Todavía|Volver|Desde|Hasta|índice|Oscuro|Atrás|Conexiones|Explorar|Técnico)\b/g) || [])];
    r.overflow = await overflow(tab);
    await shot(tab, `p${pass}-idioma-en-ficha-integral-${theme}`);
    await tab.click("header .searchbox input");
    await tab.keyboard.type("prime");
    await pause(1000);
    r.statusEN = await status(tab);
    r.resultsEN = await tab.$$eval(".results li[role=option]", (ls) => ls.slice(0, 4).map((l) => l.innerText.replace(/\n+/g, " | ")));
    const [es] = await tab.$$("xpath/.//button[normalize-space()='ES']");
    if (es) { await es.click(); await pause(1200); }
    const body2 = await text(tab);
    r.englishLeftAfterBack = [...new Set(body2.match(/\b(Search|Route|Validated|Documented|Candidate|What it is|Back|From|To|Explore|Technical|DESCRIPTION|results?)\b/g) || [])];
    r.errors = tab.errors;
    rep.lang = r;
    console.log("lang", JSON.stringify(r));
    await tab.close();
  }
  // 4. Technical mode.
  {
    const tab = await open(b, { theme });
    await go(tab, "e=wd:Q80091");
    const [tech] = await tab.$$("xpath/.//button[normalize-space()='Técnico']");
    await tech.click(); await pause(1500);
    const r = { hash: await tab.evaluate(() => location.hash) };
    const side = await text(tab, "aside.side");
    r.jargonCount = jargon(side).length;
    r.sideHead = side.slice(0, 600);
    await shot(tab, `p${pass}-tecnico-ficha-integral-${theme}`);
    await tab.reload({ waitUntil: "networkidle0" }); await pause(1000);
    r.modeAfterReload = await tab.$eval("xpath/.//button[normalize-space()='Técnico']", (x) => x.getAttribute("aria-pressed"));
    rep.technical = r;
    console.log("tech", JSON.stringify({ ...r, sideHead: r.sideHead.slice(0, 200) }));
    await tab.close();
  }
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-kb-${pass}-${theme}.json`, JSON.stringify(rep, null, 1));
