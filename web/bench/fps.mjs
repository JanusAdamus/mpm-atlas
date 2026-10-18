// FPS benchmark of the map renderer in a real browser (Edge/Chrome via puppeteer-core).
//
//   node bench/fps.mjs [--n 100000] [--secs 8] [--width 1600] [--height 900] [--browser PATH] [--page /]
//
// Serves web/dist with `vite preview`, opens `?bench=N` (N synthetic points + N edges,
// labels on, camera continuously panning and zooming) and prints the page's report
// as one JSON line: frame-interval percentiles, CPU time per frame, WebGL renderer.
import { preview } from "vite";
import puppeteer from "puppeteer-core";

const arg = (name, def) => {
  const i = process.argv.indexOf(`--${name}`);
  return i > 0 ? process.argv[i + 1] : def;
};
const n = Number(arg("n", 100000));
const secs = Number(arg("secs", 8));
const width = Number(arg("width", 1600));
const height = Number(arg("height", 900));
const page = arg("page", "/");
const browserPath = arg("browser", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe");

const server = await preview({ preview: { port: 4179, strictPort: false, open: false }, logLevel: "silent" });
const base = server.resolvedUrls.local[0];
const browser = await puppeteer.launch({
  executablePath: browserPath,
  headless: true,
  args: [
    `--window-size=${width},${height}`,
    "--enable-gpu",
    "--ignore-gpu-blocklist",
    "--use-angle=d3d11",
    "--enable-unsafe-swiftshader",
    "--disable-frame-rate-limit",
    "--disable-gpu-vsync",
  ],
  defaultViewport: { width, height, deviceScaleFactor: 1 },
});
try {
  const tab = await browser.newPage();
  tab.on("pageerror", (e) => console.error("page error:", e.message));
  await tab.goto(`${base.replace(/\/$/, "")}${page}?bench=${n}&secs=${secs}`, { waitUntil: "load" });
  await tab.waitForFunction("window.__atlasBench !== undefined", { timeout: (secs + 60) * 1000, polling: 250 });
  const report = await tab.evaluate("window.__atlasBench");
  console.log(JSON.stringify({ ...report, browser: await browser.version(), vsync: "disabled" }));
} finally {
  await browser.close();
  await server.close();
}
