// The three guided doors, keyboard only: Tab to the door, Enter, then ArrowRight through steps.
import { writeFileSync } from "node:fs";
import { launch, open, go, text, jargon, overflow, shot, OUT, pause, motion, now } from "./lib.mjs";
const [pass = "1", W = "1440", theme = "light", reduce = "0"] = process.argv.slice(2);
const w = +W, h = w < 500 ? 844 : 900;
const b = await launch();
const rep = { pass, w, theme, reduce: reduce === "1", doors: {} };
try {
  for (const door of ["door_calculus", "door_primes", "door_pythagoras"]) {
    const tab = await open(b, { w, h, theme, reduce: reduce === "1" });
    await go(tab);
    let tabs = 0, found = false;
    for (; tabs < 60; tabs++) {
      await tab.keyboard.press("Tab");
      if (await tab.evaluate((d) => document.activeElement?.dataset?.door === d, door)) { found = true; break; }
    }
    const t0 = await now(tab);
    const w0 = Date.now();
    await tab.keyboard.press("Enter");
    let ok = true;
    await tab.waitForSelector(".step-card", { visible: true, timeout: 15000 }).catch(() => { ok = false; });
    const tCard = Date.now() - w0;
    await pause(2500);
    const mo = await motion(tab, t0);
    const focus = await tab.evaluate(() => ({ cls: document.activeElement?.className, tag: document.activeElement?.tagName, text: document.activeElement?.innerText?.slice(0, 80) }));
    const card = await tab.evaluate(() => { const c = document.querySelector(".step-card"); if (!c) return null; const r = c.getBoundingClientRect(); return { top: Math.round(r.top), bottom: Math.round(r.bottom), inView: r.top < innerHeight && r.bottom > 0 }; });
    const hash = await tab.evaluate(() => location.hash);
    const result = await text(tab, ".path-result, aside.side");
    const steps = [];
    await shot(tab, `p${pass}-puerta-${door}-${w}-${theme}${reduce === "1" ? "-rm" : ""}-paso1`);
    const stopNums = await tab.evaluate(() => (window.__last || []).filter((l) => /^\d+$/.test(l.t)).map((l) => l.t));
    for (let i = 0; i < 8 && ok; i++) {
      const s = await tab.evaluate(() => {
        const c = document.querySelector(".step-card");
        return c ? { pos: c.querySelector(".step-pos")?.innerText, sentence: c.querySelector(".step-sentence")?.innerText, seal: c.querySelector(".seal")?.innerText, may: c.querySelector(".may")?.innerText, all: c.innerText } : null;
      });
      if (!s || (steps.length && steps[steps.length - 1].all === s.all)) break;
      steps.push(s);
      await tab.focus(".step-card").catch(() => {});
      const f = await tab.evaluate(() => document.activeElement?.closest?.(".step-card, .step-nav") ? "card" : document.activeElement?.className);
      await tab.keyboard.press("ArrowRight");
      await pause(700);
      s.focusBeforeArrow = f;
    }
    await shot(tab, `p${pass}-puerta-${door}-${w}-${theme}${reduce === "1" ? "-rm" : ""}-ultimo`);
    rep.doors[door] = { tabsToDoor: tabs + 1, found, cardShown: ok, tCardMs: tCard, motion: mo, focus, card, hash, stopNums, steps, jargon: jargon(result), overflow: await overflow(tab), result, errors: tab.errors };
    console.log(door, "tabs", tabs + 1, "card", ok, tCard, "ms motion", JSON.stringify(mo.episodes), "focus", focus.cls, "inView", card?.inView, "steps", steps.length, "stops", stopNums.join(","), "jargon", rep.doors[door].jargon.length, "ov", JSON.stringify(rep.doors[door].overflow), "err", tab.errors.length);
    await tab.close();
  }
} finally { await b.close(); }
writeFileSync(`${OUT}/scripts/out-doors-${pass}-${w}-${theme}-${reduce}.json`, JSON.stringify(rep, null, 1));
