// App shell: state in the URL (history/back work), map + side panel + text view.
import "./style.css";
import { api, type RouteStop, type SearchHit } from "./api.ts";
import { renderCompare } from "./compare.ts";
import { renderConcept, type ConceptActions } from "./conceptpanel.ts";
import { TILE_PX } from "./map/camera.ts";
import { clear, h } from "./dom.ts";
import { projectionLabel, setLang, t } from "./i18n.ts";
import { MapView, type MapItem, type PathMark, type RegionInfo } from "./map/map.ts";
import { renderEntity, type PanelActions } from "./panel.ts";
import { pathView, type PathQueryState } from "./pathview.ts";
import { searchBox } from "./search.ts";
import { legend, renderTextView } from "./textview.ts";
import { applyTheme, gpuTheme, initialTheme, type ThemeName } from "./theme.ts";
import { PROJECTIONS, type Lang } from "./vocab.ts";

type Panel =
  | { kind: "entity"; id: string }
  | { kind: "path"; q: PathQueryState }
  | { kind: "compare" }
  | { kind: "region"; id: string }
  | { kind: "concept"; id: string; offset: number }
  | null;

interface State {
  lang: Lang;
  layer: string;
  text: boolean;
  panel: Panel;
}

const DEFAULT_PATH: PathQueryState = { from: null, to: null, objective: "best_supported", maxHops: 6, includeDisputed: false, weights: "" };

const state: State = { lang: "es", layer: "structure", text: false, panel: null };
let theme: ThemeName = initialTheme();
let layoutVersion: string | null = null;
let maxZoom = 0;
/** Zoom semantics from /api/health: last zoom of concepts-only view, first zoom with declarations. */
let generalZoom: number | null = null;
let detailZoom: number | null = null;
let compareSet: MapItem[] = [];
let regionFocus: string | null = null;
let nodeCount: number | null = null;
const app = document.getElementById("app")!;

// ---- URL <-> state ------------------------------------------------------------------

function encode(s: State): string {
  const p = new URLSearchParams();
  if (s.lang !== "es") p.set("lang", s.lang);
  if (s.layer !== "structure") p.set("layer", s.layer);
  if (s.text) p.set("text", "1");
  const panel = s.panel;
  if (panel?.kind === "entity") p.set("e", panel.id);
  if (panel?.kind === "region") p.set("r", panel.id);
  if (panel?.kind === "concept") {
    p.set("c", panel.id);
    if (panel.offset) p.set("co", String(panel.offset));
  }
  if (panel?.kind === "compare") p.set("compare", "1");
  if (panel?.kind === "path") {
    p.set("path", "1");
    if (panel.q.from) p.set("from", `${panel.q.from.id}|${panel.q.from.label}`);
    if (panel.q.to) p.set("to", `${panel.q.to.id}|${panel.q.to.label}`);
    p.set("obj", panel.q.objective);
    p.set("hops", String(panel.q.maxHops));
    if (panel.q.includeDisputed) p.set("disputed", "1");
    if (panel.q.weights) p.set("w", panel.q.weights);
  }
  return `#${p.toString()}`;
}

function decode(hash: string): State {
  const p = new URLSearchParams(hash.replace(/^#/, ""));
  const end = (v: string | null) => {
    if (!v) return null;
    const [id, ...rest] = v.split("|");
    return { id, label: rest.join("|") || id };
  };
  let panel: Panel = null;
  if (p.get("e")) panel = { kind: "entity", id: p.get("e")! };
  else if (p.get("r")) panel = { kind: "region", id: p.get("r")! };
  else if (p.get("c")) panel = { kind: "concept", id: p.get("c")!, offset: Math.max(0, Math.floor(Number(p.get("co") ?? 0)) || 0) };
  else if (p.get("compare")) panel = { kind: "compare" };
  else if (p.get("path")) {
    panel = {
      kind: "path",
      q: {
        from: end(p.get("from")), to: end(p.get("to")), objective: p.get("obj") ?? "best_supported",
        maxHops: Number(p.get("hops") ?? 6) || 6, includeDisputed: p.get("disputed") === "1", weights: p.get("w") ?? "",
      },
    };
  }
  const lang = p.get("lang") === "en" ? "en" : "es";
  const layer = PROJECTIONS.includes(p.get("layer") ?? "") ? p.get("layer")! : "structure";
  return { lang, layer, text: p.get("text") === "1", panel };
}

function navigate(next: Partial<State>, replace = false): void {
  Object.assign(state, next);
  const url = encode(state);
  if (url !== location.hash) (replace ? history.replaceState : history.pushState).call(history, null, "", url);
  render();
}

// ---- DOM skeleton ------------------------------------------------------------------

/** A small static mark: three linked points, the atlas in miniature. */
function brandMark(): SVGSVGElement {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", "0 0 20 20");
  svg.setAttribute("class", "brand-mark");
  svg.setAttribute("aria-hidden", "true");
  const el = (tag: string, attrs: Record<string, string>): void => {
    const e = document.createElementNS(ns, tag);
    for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
    svg.append(e);
  };
  el("path", { d: "M4 15 L10 4 L16 13 Z", fill: "none", stroke: "currentColor", "stroke-width": "1.3", "stroke-linejoin": "round", opacity: "0.55" });
  el("circle", { cx: "4", cy: "15", r: "2.4", fill: "currentColor" });
  el("circle", { cx: "10", cy: "4", r: "2.4", fill: "currentColor" });
  el("circle", { cx: "16", cy: "13", r: "2.4", fill: "none", stroke: "currentColor", "stroke-width": "1.5" });
  return svg;
}

const search = searchBox({ label: t("search"), placeholder: t("searchPlaceholder"), onPick: (hit: SearchHit) => navigate({ panel: { kind: "entity", id: hit.id } }) });
const layerSelect = h("select", { id: "layer", onchange: () => navigate({ layer: layerSelect.value }, true) });
const langBtn = h("button", { type: "button", class: "chip", onclick: () => navigate({ lang: state.lang === "es" ? "en" : "es" }, true) });
const themeBtn = h("button", { type: "button", class: "chip", onclick: () => setTheme(theme === "dark" ? "light" : "dark") });
const textBtn = h("button", { type: "button", class: "chip", "aria-pressed": "false", onclick: () => navigate({ text: !state.text }) });
const pathBtn = h("button", { type: "button", class: "chip", onclick: () => navigate({ panel: { kind: "path", q: { ...DEFAULT_PATH } } }) });
const compareBtn = h("button", { type: "button", class: "chip", onclick: () => navigate({ panel: { kind: "compare" } }) });
const backBtn = h("button", { type: "button", class: "chip", onclick: () => history.back() });
const titleEl = h("span", { class: "title" });
const layerLabel = h("label", { for: "layer", class: "sr-only" });
compareBtn.classList.add("wide");
backBtn.classList.add("wide");
const sep = (): HTMLElement => h("span", { class: "sep", "aria-hidden": "true" });
const tools = h("div", { class: "tools" }, layerLabel, layerSelect, sep(), pathBtn, compareBtn, textBtn, sep(), langBtn, themeBtn, backBtn);
const header = h("header", { class: "top" }, h("h1", {}, brandMark(), titleEl), search.root, tools);
const mapHost = h("div", { class: "map-host", id: "map" });
const focusChip = h("button", { type: "button", class: "chip focus-chip", hidden: true, onclick: () => setRegionFocus(null, null) });
const zoomIn = h("button", { type: "button", class: "zbtn", onclick: () => zoomBy(1.6) }, "+");
const zoomOut = h("button", { type: "button", class: "zbtn", onclick: () => zoomBy(1 / 1.6) }, "−");
const zoomReset = h("button", { type: "button", class: "zbtn", onclick: () => map.resetView() }, "⤢");
const status = h("div", { class: "status", role: "status", "aria-live": "polite" });
const help = h("p", { class: "help small" });
const side = h("aside", { class: "side", "aria-live": "polite", tabindex: -1 });
const textPane = h("section", { class: "textpane", "aria-label": "" });
const mainEl = h("main", {}, textPane, h("div", { class: "map-wrap" }, mapHost, h("div", { class: "zoom" }, zoomIn, zoomOut, zoomReset), focusChip, status, help), side);
app.append(header, mainEl);

const map = new MapView(mapHost, {
  onSelect: (it, shift) => {
    if (shift) addCompare(it);
    else if (it.kind === "entity") navigate({ panel: { kind: "entity", id: it.id } });
    else if (it.kind === "concept") navigate({ panel: { kind: "concept", id: it.id, offset: 0 } });
    else {
      map.flyTo(it.x, it.y, map.camera.scale * 3);
      selectedRegion = map.region(it);
      navigate({ panel: { kind: "region", id: it.id } });
    }
  },
  onViewChange: () => scheduleText(),
});
let selectedRegion: RegionInfo | null = null;

function zoomBy(f: number): void {
  const c = map.camera;
  c.flyTo({ ...c.view(), scale: c.scale * f }, 220);
  map.invalidate();
}

function setTheme(name: ThemeName): void {
  theme = name;
  applyTheme(name);
  map.setTheme(gpuTheme(name), name === "dark");
  renderChrome();
}

function addCompare(it: MapItem): void {
  compareSet = [...compareSet.filter((x) => x.id !== it.id), it].slice(-2);
  map.setCompare(compareSet);
  navigate({ panel: { kind: "compare" } });
}

async function setRegionFocus(id: string | null, label: string | null): Promise<void> {
  regionFocus = id;
  focusChip.hidden = !id;
  if (!id || !layoutVersion) {
    map.setFocus(null);
    return;
  }
  try {
    const loc = await api.locate(id, layoutVersion);
    map.setFocus(loc.communities);
    map.flyTo(loc.x, loc.y);
    focusChip.textContent = `◎ ${label ?? id} ✕`;
  } catch (err) {
    status.textContent = `${t("error")}: ${(err as Error).message}`;
  }
}

// ---- side panel ------------------------------------------------------------------

let panelCtrl: AbortController | null = null;

const entityActions: PanelActions = {
  open: (id) => navigate({ panel: { kind: "entity", id } }),
  showOnMap: (id) => void locateAndSelect(id, true),
  focusRegion: (id) => void setRegionFocus(id, side.querySelector("#entity-title")?.textContent ?? id),
  pathFrom: (id, label) => navigate({ panel: { kind: "path", q: { ...pathState(), from: { id, label } } } }),
  pathTo: (id, label) => navigate({ panel: { kind: "path", q: { ...pathState(), to: { id, label } } } }),
  openConcept: (id) => navigate({ panel: { kind: "concept", id, offset: 0 } }),
};

/** Camera scale at which the map fetches tiles of zoom `z` (camera.tileZoom rounds log2(scale / TILE_PX)). */
const scaleForZoom = (z: number): number => TILE_PX * 2 ** z * 1.05;

/** Radius (unit square) of the disc a concept's declarations are drawn in: atlas-layout's
 *  MEMBER_DISC (5e-4) times the square root of its members. */
const memberDisc = (members: number): number => 5e-4 * Math.sqrt(Math.max(1, members));

/** Fly to a concept: unfolded (at least `detailZoom`, with its disc filling about a third
 *  of the view) or folded (concepts only, `generalZoom`). */
async function flyToConcept(id: string, unfold: boolean, members = 1): Promise<void> {
  if (!layoutVersion) return;
  try {
    const loc = await api.locate(id, layoutVersion);
    map.setSelected({ x: loc.x, y: loc.y });
    const c = map.camera;
    const scale = unfold
      ? Math.min(scaleForZoom(maxZoom), Math.max(scaleForZoom(Math.min(detailZoom ?? maxZoom, maxZoom)), (0.35 * Math.min(c.width, c.height)) / memberDisc(members)))
      : scaleForZoom(Math.min(generalZoom ?? 0, maxZoom));
    map.flyTo(loc.x, loc.y, scale);
  } catch (err) {
    status.textContent = `${t("error")}: ${(err as Error).message}`;
  }
}

const conceptActions = (offsetBase: number): ConceptActions => ({
  open: (id) => navigate({ panel: { kind: "entity", id } }),
  openConcept: (id) => navigate({ panel: { kind: "concept", id, offset: 0 } }),
  expand: (id, members) => void flyToConcept(id, true, members),
  collapse: (id) => void flyToConcept(id, false),
  page: (offset) => {
    const p = state.panel;
    if (p?.kind === "concept" && offset !== offsetBase) navigate({ panel: { ...p, offset } }, true);
  },
  drill: (from, to, offset) => api.conceptEdges(from, to, state.lang, offset, 20),
});

function pathState(): PathQueryState {
  return state.panel?.kind === "path" ? state.panel.q : { ...DEFAULT_PATH };
}

async function locateAndSelect(id: string, fly: boolean): Promise<void> {
  if (!layoutVersion) return;
  try {
    const loc = await api.locate(id, layoutVersion);
    map.setSelected({ x: loc.x, y: loc.y });
    if (fly) map.flyTo(loc.x, loc.y);
  } catch {
    map.setSelected(null);
  }
}

async function showPath(steps: RouteStop[], historical = false): Promise<void> {
  if (!steps.length) {
    map.setPath(null);
    return;
  }
  if (!layoutVersion) return;
  const ids = [steps[0].from, ...steps.map((s) => s.to)];
  const labels = [steps[0].from_label, ...steps.map((s) => s.to_label)];
  try {
    const locs = await Promise.all(ids.map((id) => api.locate(id, layoutVersion!)));
    const marks: PathMark[] = locs.map((l, i) => ({ id: l.id, label: labels[i], x: l.x, y: l.y }));
    map.setPath(marks, historical);
  } catch (err) {
    status.textContent = `${t("error")}: ${(err as Error).message}`;
  }
}

async function renderPanel(): Promise<void> {
  panelCtrl?.abort();
  panelCtrl = new AbortController();
  const p = state.panel;
  side.hidden = !p;
  mainEl.classList.toggle("with-side", !!p);
  if (p?.kind !== "path") map.setPath(null);
  if (!p) {
    clear(side);
    map.setSelected(null);
    return;
  }
  const closeBtn = h("button", { type: "button", class: "close", "aria-label": t("close"), onclick: () => navigate({ panel: null }) }, "✕");
  if (p.kind === "entity") {
    side.replaceChildren(closeBtn, h("p", { class: "muted" }, t("loading")));
    try {
      const r = await api.entity(p.id, state.lang, panelCtrl.signal);
      side.replaceChildren(closeBtn, renderEntity(r, entityActions));
      (side.querySelector("#entity-title") as HTMLElement | null)?.focus();
      document.title = `${r.label} · Atlas`;
      void locateAndSelect(r.canonical_id, false);
    } catch (err) {
      if ((err as Error).name === "AbortError") return;
      side.replaceChildren(closeBtn, h("p", { class: "notice error", role: "alert" }, `${t("error")}: ${(err as Error).message}`));
    }
  } else if (p.kind === "path") {
    side.replaceChildren(closeBtn, pathView(p.q, {
      open: (id) => navigate({ panel: { kind: "entity", id } }),
      openConcept: (id) => navigate({ panel: { kind: "concept", id, offset: 0 } }),
      showPath: (steps, historical) => void showPath(steps, historical),
      changed: (q) => {
        state.panel = { kind: "path", q };
        history.replaceState(null, "", encode(state));
      },
    }));
  } else if (p.kind === "compare") {
    const [a, b] = compareSet.map((it) => map.region(it));
    const between = a && b ? map.edgesBetween(a.item, b.item) : new Map<string, number>();
    side.replaceChildren(closeBtn, renderCompare(a ?? null, b ?? null, between, entityActions.open, (r) => void setRegionFocus(r.central[0] ?? null, r.item.label)));
  } else if (p.kind === "concept") {
    side.replaceChildren(closeBtn, h("p", { class: "muted" }, t("loading")));
    try {
      const r = await api.concept(p.id, state.lang, p.offset, 50, panelCtrl.signal);
      side.replaceChildren(closeBtn, renderConcept(r, conceptActions(p.offset)));
      (side.querySelector("#entity-title") as HTMLElement | null)?.focus();
      document.title = `${r.concept.label} · Atlas`;
      void locateAndSelect(r.concept.id, false);
    } catch (err) {
      if ((err as Error).name === "AbortError") return;
      side.replaceChildren(closeBtn, h("p", { class: "notice error", role: "alert" }, `${t("error")}: ${(err as Error).message}`));
    }
  } else if (p.kind === "region") {
    const r = selectedRegion?.item.id === p.id ? selectedRegion : (() => {
      const it = map.findItem(p.id);
      return it ? map.region(it) : null;
    })();
    side.replaceChildren(closeBtn, r
      ? h("div", {},
        renderCompare(r, null, new Map(), entityActions.open, (x) => void setRegionFocus(x.central[0] ?? null, x.item.label)),
        h("button", { type: "button", onclick: () => addCompare(r.item) }, t("compareWith")))
      : h("p", { class: "muted" }, p.id));
  }
}

// ---- text view -------------------------------------------------------------------

let textTimer = 0;
function scheduleText(): void {
  if (!state.text) return;
  clearTimeout(textTimer);
  textTimer = window.setTimeout(renderText, 250);
}

function renderText(): void {
  textPane.hidden = !state.text;
  mainEl.classList.toggle("with-text", state.text);
  if (!state.text) return;
  const items = map.visibleItems();
  textPane.replaceChildren(h("h2", {}, t("textView")), renderTextView(items, map.hiddenEdges(), {
    open: (it) => navigate({ panel: { kind: "entity", id: it.id } }),
    openConcept: (it) => navigate({ panel: { kind: "concept", id: it.id, offset: 0 } }),
    zoomInto: (it) => {
      if (it.kind === "concept") {
        void flyToConcept(it.id, true, it.size);
        navigate({ panel: { kind: "concept", id: it.id, offset: 0 } });
        return;
      }
      map.flyTo(it.x, it.y, map.camera.scale * 3);
      selectedRegion = map.region(it);
      navigate({ panel: { kind: "region", id: it.id } });
    },
    compare: (it) => addCompare(it),
  }, map.drawnEdges()));
}

// ---- chrome ---------------------------------------------------------------------

function renderChrome(): void {
  setLang(state.lang);
  titleEl.textContent = t("title");
  search.input.placeholder = t("searchPlaceholder");
  layerLabel.textContent = t("layer");
  layerSelect.replaceChildren(...PROJECTIONS.map((p) => h("option", { value: p, selected: p === state.layer }, `${t("layer")}: ${projectionLabel(p)}`)));
  langBtn.textContent = state.lang === "es" ? "EN" : "ES";
  langBtn.setAttribute("aria-label", `${t("language")}: ${state.lang === "es" ? "English" : "Español"}`);
  themeBtn.textContent = theme === "dark" ? `☀ ${t("themeLight")}` : `☾ ${t("themeDark")}`;
  textBtn.textContent = state.text ? t("mapView") : t("textView");
  textBtn.setAttribute("aria-pressed", String(state.text));
  pathBtn.textContent = t("path");
  compareBtn.textContent = t("compare");
  backBtn.textContent = `← ${t("back")}`;
  zoomIn.setAttribute("aria-label", t("zoomIn"));
  zoomOut.setAttribute("aria-label", t("zoomOut"));
  zoomReset.setAttribute("aria-label", t("resetView"));
  help.replaceChildren(...t("help").split(" · ").map((part) => {
    const i = part.indexOf(":");
    return i > 0 ? h("span", {}, h("kbd", {}, part.slice(0, i).trim()), ` ${part.slice(i + 1).trim()}`) : h("span", {}, part);
  }));
  if (nodeCount !== null) status.textContent = t("nodesCount", { n: nodeCount.toLocaleString(state.lang) });
  map.canvas.setAttribute("aria-label", `${t("title")} — ${t("help")}`);
  textPane.setAttribute("aria-label", t("textView"));
}

function render(): void {
  renderChrome();
  if (layoutVersion) map.setParams(state.layer, state.lang, layoutVersion, maxZoom);
  void renderPanel();
  renderText();
}

// ---- keyboard --------------------------------------------------------------------

document.addEventListener("keydown", (e) => {
  const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement;
  if (e.altKey && e.key === "ArrowLeft") {
    e.preventDefault();
    history.back();
    return;
  }
  if (typing) return;
  if (e.key === "/") {
    e.preventDefault();
    search.input.focus();
  } else if (e.key === "t" || e.key === "T") {
    navigate({ text: !state.text });
  } else if (e.key === "Escape") {
    if (regionFocus) void setRegionFocus(null, null);
    else if (state.panel) navigate({ panel: null });
    map.setPath(null);
  }
});

window.addEventListener("popstate", () => {
  Object.assign(state, decode(location.hash));
  render();
});

// ---- boot ------------------------------------------------------------------------

async function boot(): Promise<void> {
  Object.assign(state, decode(location.hash));
  applyTheme(theme);
  map.setTheme(gpuTheme(theme), theme === "dark");
  renderChrome();
  side.hidden = true;
  textPane.hidden = true;
  const bench = new URLSearchParams(location.search).get("bench");
  if (bench) {
    const n = Math.min(2_000_000, Math.max(1000, Number(bench) || 100_000));
    const secs = Number(new URLSearchParams(location.search).get("secs") ?? 8);
    const r = await map.bench(n, secs);
    (window as unknown as { __atlasBench: unknown }).__atlasBench = r;
    status.textContent = JSON.stringify(r);
    return;
  }
  try {
    const health = await api.health();
    layoutVersion = health.layout_version;
    generalZoom = health.map?.general_zoom ?? null;
    detailZoom = health.map?.detail_zoom ?? null;
    nodeCount = health.snapshot.nodes;
    status.textContent = t("nodesCount", { n: nodeCount.toLocaleString(state.lang) });
    status.title = map.rendererName;
    if (layoutVersion) {
      const root = await api.tile(state.layer, 0, 0, 0, state.lang, layoutVersion);
      maxZoom = root.maxZoom;
    } else {
      mapHost.append(h("p", { class: "notice map-notice" }, t("noLayout")));
      textPane.append(legend());
    }
  } catch (err) {
    status.textContent = `${t("error")}: ${(err as Error).message}`;
  }
  render();
}

void boot();
