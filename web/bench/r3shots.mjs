// Round 3 browser check: the concept route in the path panel, the drill-down to
// declarations by keyboard, the text view and the phone width.  Screenshots go to --out.
//
//   node bench/r3shots.mjs --base http://127.0.0.1:8769 --from "wd:Q1|A" --to "wd:Q2|B" --out ../docs/design/r3
//
// Prints one JSON line; exits 1 on a page or console error, an HTTP status >= 400, a
// missing element, a `p=` in the visible text or horizontal overflow on the phone.
import puppeteer from "puppeteer-core";

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const base = arg("base", "http://127.0.0.1:8769");
const from = arg("from", null);
const to = arg("to", null);
const out = arg("out", "../docs/design/r3");
const browserPath = arg("browser", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe");
const pathHash = `#${new URLSearchParams({ path: "1", from, to, obj: "best_supported", hops: "6" })}`;

const errors = [];
const report = {};
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
const noP = async (tab, view) => {
  if (/\bp=/.test(await tab.evaluate(() => document.body.innerText))) errors.push(`${view}: "p=" in the visible text`);
};
try {
  const tab = await browser.newPage();
  watch(tab);
  await tab.setViewport({ width: 1400, height: 900 });
  await tab.goto(base, { waitUntil: "networkidle0" });
  await tab.screenshot({ path: `${out}/01-overview.png` });
  await noP(tab, "overview");

  await tab.goto(`${base}/${pathHash}`, { waitUntil: "networkidle0" });
  await tab.waitForSelector(".concept-route", { visible: true, timeout: 15000 });
  report.concept_steps = await tab.$$eval(".concept-route > .route > .hop", (els) => els.length);
  report.declaration_paths = await tab.$$eval(".path-result > .path:not(.concept-route)", (els) => els.length);
  report.abstained = await tab.$$eval(".path-result > .notice", (els) => els.length > 0);
  await tab.screenshot({ path: `${out}/02-concept-route.png` });
  await noP(tab, "concept route");

  // Keyboard only: focus the first drill-down button and press Enter.
  await tab.focus(".drill-toggle");
  await tab.keyboard.press("Enter");
  await tab.waitForSelector(".drill:not([hidden]) .route .hop", { visible: true, timeout: 5000 });
  report.drill_expanded = await tab.$eval(".drill-toggle", (el) => el.getAttribute("aria-expanded"));
  report.drill_hops = await tab.$$eval(".drill:not([hidden]) .route .hop", (els) => els.length);
  await tab.$eval(".drill:not([hidden])", (el) => el.scrollIntoView({ block: "center" }));
  await tab.screenshot({ path: `${out}/03-drill-down.png` });
  await tab.focus(".drill-toggle");
  await tab.keyboard.press("Enter");
  report.drill_collapsed = await tab.$eval(".drill-toggle", (el) => el.getAttribute("aria-expanded") === "false");

  await tab.keyboard.press("Escape");
  await tab.evaluate(() => (document.activeElement)?.blur());
  await tab.keyboard.press("t");
  await tab.waitForSelector(".textpane:not([hidden]) h2", { timeout: 5000 });
  report.text_view_items = await tab.$$eval(".textpane li", (els) => els.length);
  await tab.screenshot({ path: `${out}/04-text-view.png` });
  await noP(tab, "text view");

  const phone = await browser.newPage();
  watch(phone);
  await phone.setViewport({ width: 390, height: 844, isMobile: true, hasTouch: true });
  await phone.goto(`${base}/${pathHash}`, { waitUntil: "networkidle0" });
  await phone.waitForSelector(".concept-route", { visible: true, timeout: 15000 });
  report.phone_overflow_px = await phone.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
  if (report.phone_overflow_px > 0) errors.push(`phone: ${report.phone_overflow_px} px of horizontal overflow`);
  await phone.$eval(".concept-route", (el) => el.scrollIntoView({ block: "start" }));
  await phone.screenshot({ path: `${out}/05-concept-route-mobile.png` });
  await noP(phone, "phone");
} catch (e) {
  errors.push(`r3shots: ${e.message}`);
} finally {
  await browser.close();
}
report.errors = errors;
console.log(JSON.stringify(report));
process.exit(errors.length ? 1 : 0);
