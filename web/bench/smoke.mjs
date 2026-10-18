// End-to-end smoke test of the built client against a running `atlas serve`.
//
//   node bench/smoke.mjs [--base http://127.0.0.1:8765] [--q pitagoras] [--shot out.png] [--browser PATH]
//
// Opens the app headless, waits for map tiles, types a query, picks the first hit,
// waits for the explanation panel, toggles the text view and EN, and prints one JSON
// line with timings.  Exits 1 on any page error, console error or missing element.
import puppeteer from "puppeteer-core";

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const base = arg("base", "http://127.0.0.1:8765");
const q = arg("q", "pitagoras");
const shot = arg("shot", null);
const browserPath = arg("browser", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe");

const errors = [];
const browser = await puppeteer.launch({
  executablePath: browserPath,
  headless: true,
  args: ["--window-size=1400,900", "--enable-gpu", "--ignore-gpu-blocklist", "--use-angle=d3d11", "--enable-unsafe-swiftshader"],
  defaultViewport: { width: 1400, height: 900 },
});
const report = {};
try {
  const tab = await browser.newPage();
  tab.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  tab.on("console", (m) => m.type() === "error" && errors.push(`console: ${m.text()}`));
  const tiles = [];
  tab.on("response", (r) => r.url().includes("/api/map/tiles/") && tiles.push(r.status()));
  let t = performance.now();
  await tab.goto(base, { waitUntil: "networkidle0" });
  report.load_ms = Math.round(performance.now() - t);
  report.tiles = tiles.length;
  report.tile_status = [...new Set(tiles)];
  report.status = await tab.$eval(".status", (el) => el.textContent);

  t = performance.now();
  await tab.focus("input[type=search]");
  await tab.keyboard.type(q, { delay: 20 });
  await tab.waitForSelector(".results [role=option]", { visible: true, timeout: 10000 });
  report.search_ms = Math.round(performance.now() - t);
  report.hits = await tab.$$eval(".results [role=option]", (els) => els.length);
  report.first_hit = await tab.$eval(".results [role=option] .hit-label", (el) => el.textContent);

  t = performance.now();
  await tab.keyboard.press("Enter"); // the first hit is already highlighted
  await tab.waitForSelector("#entity-title", { visible: true, timeout: 10000 });
  report.entity_ms = Math.round(performance.now() - t);
  report.entity = await tab.$eval("#entity-title", (el) => el.textContent);
  if (report.entity !== report.first_hit) errors.push(`Enter opened ${report.entity}, not the first hit ${report.first_hit}`);
  report.claim_lines = await tab.$$eval(".claim-line", (els) => els.length);
  report.badges = await tab.$$eval(".badge", (els) => els.length);

  await tab.keyboard.press("Escape");
  await tab.keyboard.press("t");
  await tab.waitForSelector(".textpane:not([hidden]) h2", { timeout: 5000 });
  report.text_view_items = await tab.$$eval(".textpane li", (els) => els.length);
  await tab.click("header button.chip:nth-of-type(4)"); // path, compare, text, language
  report.hash = await tab.evaluate(() => location.hash);
  if (!report.hash.includes("lang=en")) errors.push(`language toggle did not reach the URL: ${report.hash}`);
  report.title_en = await tab.$eval("header h1", (el) => el.textContent);
  // EN tiles reload, then the text view re-renders (250 ms debounce): it must follow the language.
  await tab.waitForFunction(
    () => !document.querySelector(".textpane")?.textContent?.includes("Relaciones desconocidas"),
    { timeout: 5000 },
  ).catch(() => errors.push("text view kept Spanish region labels after switching to EN"));
  report.text_view_items_en = await tab.$$eval(".textpane li", (els) => els.length);
  if (shot) await tab.screenshot({ path: shot });
} catch (e) {
  errors.push(`smoke: ${e.message}`);
} finally {
  await browser.close();
}
report.errors = errors;
console.log(JSON.stringify(report));
process.exit(errors.length ? 1 : 0);
