// App shell: state in the URL (history/back work), map + side panel + text view.
// Round 4: the side panel opens on the entry (a question, the search box and three guided
// doors), the reading mode (Explorar by default, Técnico) is a visible switch remembered in
// localStorage, and the panel enters in 200 ms and leaves in 150 ms (a fade under reduced motion).
import "./style.css";
import "./panels.css";
import { api, type RouteStop, type SearchHit } from "./api.ts";
import { renderCompare } from "./compare.ts";
import { renderConcept, type ConceptActions } from "./conceptpanel.ts";
import { contentBounds, TILE_PX } from "./map/camera.ts";
import { h } from "./dom.ts";
import { legendList } from "./evidence.ts";
import { isExplore, projectionLabel, setLang, setMode, t, type Mode } from "./i18n.ts";
import { MapView, type MapItem, type PathMark, type RegionInfo } from "./map/map.ts";
import { mergeEntityPage, renderEntity, type PanelActions } from "./panel.ts";
import { pathView, readerLabel, type PathQueryState } from "./pathview.ts";
import { searchBox } from "./search.ts";
import { focusReturn } from "./textfocus.ts";
import { legend, renderTextView } from "./textview.ts";
import { applyTheme, gpuTheme, initialTheme, type ThemeName } from "./theme.ts";
import { MOTION, REDUCED_MOTION, prefersReducedMotion } from "./tokens.ts";
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
  mode: Mode;
}

/** Guided doors: routes between concepts that exist in the snapshot (ids found with
 *  /api/search on 2026-10-19: derivada wd:Q29175, integración wd:Q80091, número primo
 *  wd:Q49008, criptografía wd:Q8789, teorema de Pitágoras wd:Q11518, trigonometría wd:Q8084). */
export const DOORS = [
  { key: "door_calculus", from: "wd:Q29175", to: "wd:Q80091" },
  { key: "door_primes", from: "wd:Q49008", to: "wd:Q8789" },
  { key: "door_pythagoras", from: "wd:Q11518", to: "wd:Q8084" },
] as const;

const DEFAULT_PATH: PathQueryState = { from: null, to: null, objective: "best_supported", maxHops: 6, includeDisputed: false, weights: "" };
const MODE_KEY = "atlas.mode";

function storedMode(): Mode {
  try {
    return localStorage.getItem(MODE_KEY) === "technical" ? "technical" : "explore";
  } catch {
    return "explore"; // storage blocked: the default
  }
}
function storeMode(m: Mode): void {
  try {
    localStorage.setItem(MODE_KEY, m);
  } catch {
    // Not persisted; the choice still applies to this page.
  }
}

const state: State = { lang: "es", layer: "structure", text: false, panel: null, mode: storedMode() };
let theme: ThemeName = initialTheme();
let layoutVersion: string | null = null;
let maxZoom = 0;
/** Zoom semantics from /api/health: last zoom of concepts-only view, first zoom with declarations. */
let generalZoom: number | null = null;
let detailZoom: number | null = null;
let compareSet: MapItem[] = [];
let regionFocus: string | null = null;
let nodeCount: number | null = null;
/** The last route the reader opened, for «Volver a mi ruta». */
let myRoute: PathQueryState | null = null;
/** A guided door was just opened: the step-by-step card takes focus when it arrives. */
let doorFocus = false;
const app = document.getElementById("app")!;

// ---- URL <-> state ------------------------------------------------------------------

function encode(s: State): string {
  const p = new URLSearchParams();
  if (s.lang !== "es") p.set("lang", s.lang);
  if (s.mode === "technical") p.set("m", "tecnico");
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
    if (panel.q.step) p.set("st", String(panel.q.step));
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
        step: Math.max(0, Math.floor(Number(p.get("st") ?? 0)) || 0),
      },
    };
  }
  const lang = p.get("lang") === "en" ? "en" : "es";
  const layer = PROJECTIONS.includes(p.get("layer") ?? "") ? p.get("layer")! : "structure";
  const mode: Mode = p.get("m") === "tecnico" ? "technical" : p.get("m") === "explorar" ? "explore" : storedMode();
  return { lang, layer, text: p.get("text") === "1", panel, mode };
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

const openHit = (hit: SearchHit): void => openIdea(hit.id, hit.type === "concept" ? "concept" : "entity");
const search = searchBox({ label: t("search"), placeholder: t("searchPlaceholder"), onPick: openHit });
const homeSearch = searchBox({ label: t("ask"), placeholder: t("searchPlaceholder"), onPick: openHit });
const layerSelect = h("select", { id: "layer", onchange: () => navigate({ layer: layerSelect.value }, true) });
const langBtn = h("button", { type: "button", class: "chip", onclick: () => navigate({ lang: state.lang === "es" ? "en" : "es" }, true) });
const themeBtn = h("button", { type: "button", class: "chip", onclick: () => setTheme(theme === "dark" ? "light" : "dark") });
const textBtn = h("button", { type: "button", class: "chip", "aria-pressed": "false", onclick: () => toggleText() });
const pathBtn = h("button", { type: "button", class: "chip", onclick: () => navigate({ panel: { kind: "path", q: { ...DEFAULT_PATH } } }) });
const compareBtn = h("button", { type: "button", class: "chip", onclick: () => navigate({ panel: { kind: "compare" } }) });
const backBtn = h("button", { type: "button", class: "chip", onclick: () => history.back() });
const modeExplore = h("button", { type: "button", class: "chip", onclick: () => setReadingMode("explore") });
const modeTech = h("button", { type: "button", class: "chip", onclick: () => setReadingMode("technical") });
const modeSwitch = h("div", { class: "mode-switch", role: "group" }, modeExplore, modeTech);
const titleEl = h("span", { class: "title" });
const layerLabel = h("label", { for: "layer", class: "sr-only" });
compareBtn.classList.add("wide");
backBtn.classList.add("wide");
const sep = (): HTMLElement => h("span", { class: "sep", "aria-hidden": "true" });
const layerSep = sep();
const tools = h("div", { class: "tools" }, modeSwitch, sep(), layerLabel, layerSelect, layerSep, pathBtn, compareBtn, textBtn, sep(), langBtn, themeBtn, backBtn);
const header = h("header", { class: "top" }, h("h1", {}, brandMark(), titleEl), search.root, tools);
const mapHost = h("div", { class: "map-host", id: "map" });
const focusChip = h("button", { type: "button", class: "chip focus-chip", hidden: true, onclick: () => setRegionFocus(null, null) });
const zoomIn = h("button", { type: "button", class: "zbtn", onclick: () => zoomBy(1.6) }, "+");
const zoomOut = h("button", { type: "button", class: "zbtn", onclick: () => zoomBy(1 / 1.6) }, "−");
const zoomReset = h("button", { type: "button", class: "zbtn", onclick: () => map.resetView() }, "⤢");
const status = h("div", { class: "status", role: "status", "aria-live": "polite" });
const help = h("p", { class: "help small" });
/** The legend on the map: every seal once, «desconocida» included.  Its title is always in
 *  view; on a phone it starts folded so it does not cover a third of the map (C4-12). */
const PHONE = "(max-width: 760px)";
const mapLegend = h("details", { class: "map-legend", open: !matchMedia(PHONE).matches });
const legendSummary = h("summary", {});
const legendBody = h("div", { role: "group" });
mapLegend.append(legendSummary, legendBody);
const side = h("aside", { class: "side", tabindex: -1 });
const textPane = h("section", { class: "textpane", "aria-labelledby": "textpane-title" });
const textTitle = h("h2", { id: "textpane-title", tabindex: -1 });
const textBody = h("div", {});
textPane.append(textTitle, textBody);
const textFocus = focusReturn(() => document.activeElement as HTMLElement | null);
const mainEl = h("main", {}, textPane, h("div", { class: "map-wrap" }, mapHost, h("div", { class: "zoom" }, zoomIn, zoomOut, zoomReset), focusChip, mapLegend, status, help), side);
/** «Saltar a las puertas» (or to the open panel): the first example is two Tabs and Enter away
 *  instead of sixteen Tabs through the header and the map controls (C4-18). */
const skipLink = h("a", { class: "skip", href: "#", onclick: (e: Event) => {
  e.preventDefault();
  const target = side.querySelector<HTMLElement>(".door") ?? side.querySelector<HTMLElement>("h2") ?? side;
  if (!target.hasAttribute("tabindex") && !/^(BUTTON|A|INPUT)$/.test(target.tagName)) target.setAttribute("tabindex", "-1");
  target.focus();
} });
app.append(skipLink, header, mainEl);

const map = new MapView(mapHost, {
  onSelect: (it, shift) => {
    if (shift) addCompare(it);
    else if (it.kind === "entity") navigate({ panel: { kind: "entity", id: it.id } });
    else if (it.kind === "concept") openIdea(it.id, "concept");
    else {
      map.flyTo(it.x, it.y, map.camera.scale * 3);
      selectedRegion = map.region(it);
      navigate({ panel: { kind: "region", id: it.id } });
    }
  },
  onViewChange: () => scheduleText(),
});
let selectedRegion: RegionInfo | null = null;

/** The map's round-4 API (map/map.ts): reading mode, one protagonist with its neighbors
 *  and framed flights.  setSelected is kept for points with no focus. */
const mapAdapter = {
  setMode: (m: Mode): void => map.setMode(m),
  focus: (id: string | null, pos?: { x: number; y: number; label?: string }, neighbors?: string[]): void => {
    map.focus(id, id ? { ...pos, neighbors } : {});
    map.setSelected(id ? null : pos ?? null);
  },
  frame: (pts: { x: number; y: number }[]): void => {
    if (!pts.length) return;
    const xs = pts.map((p) => p.x), ys = pts.map((p) => p.y);
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    const pad = Math.max(x1 - x0, y1 - y0, 2e-3) * 0.25;
    map.flyToBounds(x0 - pad, y0 - pad, x1 + pad, y1 + pad);
  },
};

function zoomBy(f: number): void {
  const c = map.camera;
  c.flyTo({ ...c.view(), scale: c.scale * f }, MOTION.camera);
  map.invalidate();
}

function setTheme(name: ThemeName): void {
  theme = name;
  applyTheme(name);
  map.setTheme(gpuTheme(name), name === "dark");
  renderChrome();
}

function setReadingMode(m: Mode): void {
  storeMode(m);
  navigate({ mode: m }, true);
}

/** Open an idea: in the explore mode a concept opens its five-question record (a Wikidata
 *  item); the grouped code list is the technical view. */
function openIdea(id: string, kind: "entity" | "concept"): void {
  if (kind === "concept" && (!isExplore() || !id.startsWith("wd:"))) navigate({ panel: { kind: "concept", id, offset: 0 } });
  else navigate({ panel: { kind: "entity", id } });
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
  pathFrom: (id, label) => navigate({ panel: { kind: "path", q: { ...pathState(), from: { id, label }, step: 0 } } }),
  pathTo: (id, label) => navigate({ panel: { kind: "path", q: { ...pathState(), to: { id, label }, step: 0 } } }),
  // The grouped code is technical: the explore button says so, and switches mode.
  openConcept: (id) => {
    if (isExplore()) storeMode("technical");
    navigate({ mode: "technical", panel: { kind: "concept", id, offset: 0 } });
  },
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
  technical: () => setReadingMode("technical"),
});

function pathState(): PathQueryState {
  return state.panel?.kind === "path" ? state.panel.q : { ...DEFAULT_PATH };
}

/** Neighbors for the map's focus, best first, from the API (the tiles alone miss most of
 *  them): established before candidates, ideas before code records. */
export function neighborIds(claims: { relation: string; evidence_state: string; participants: { entity: string }[] }[], self: string): string[] {
  const rank = (c: (typeof claims)[number]): number => (c.evidence_state === "model_proposed" || c.evidence_state === "disputed" ? 1 : 0);
  const ids = [...claims].sort((a, b) => rank(a) - rank(b)).flatMap((c) => c.participants.map((p) => p.entity))
    .filter((e) => e !== self && !e.startsWith("mathlib:") && !e.startsWith("wp:"));
  return [...new Set(ids)];
}

async function locateAndSelect(id: string, fly: boolean, label?: string, neighbors?: string[]): Promise<void> {
  if (!layoutVersion) return;
  try {
    const loc = await api.locate(id, layoutVersion);
    mapAdapter.focus(id, { x: loc.x, y: loc.y, label }, neighbors);
    if (fly) map.flyTo(loc.x, loc.y);
  } catch {
    mapAdapter.focus(null);
  }
}

/** The route on the map: its stops (located once) and the step the reader is on. */
let routeView: { key: string; locs: { id: string; label: string; x: number; y: number }[] | null; step: number } = { key: "", locs: null, step: 0 };

function frameStep(i: number): void {
  const l = routeView.locs;
  if (!l || !l[i] || !l[i + 1]) return;
  mapAdapter.focus(l[i + 1].id, l[i + 1]);
  mapAdapter.frame([l[i], l[i + 1]]);
}

async function showPath(steps: RouteStop[], historical = false): Promise<void> {
  if (!steps.length) {
    map.setPath(null);
    routeView = { key: "", locs: null, step: 0 };
    return;
  }
  if (!layoutVersion) return;
  const ids = [steps[0].from, ...steps.map((s) => s.to)];
  const labels = [steps[0].from_label, ...steps.map((s) => s.to_label)];
  const key = ids.join(">");
  if (routeView.key !== key) routeView = { key, locs: null, step: routeView.key === "" ? routeView.step : 0 };
  try {
    const locs = await Promise.all(ids.map((id) => api.locate(id, layoutVersion!)));
    if (routeView.key !== key) return; // a newer route arrived meanwhile
    const marks: PathMark[] = locs.map((l, i) => ({ id: l.id, label: readerLabel(labels[i]), x: l.x, y: l.y, state: i > 0 ? (steps[i - 1] as { evidence_state?: string }).evidence_state : undefined }));
    routeView.locs = marks;
    map.setPath(marks, historical);
    // The camera flies to the whole route's region; a later step frames its own hop.
    if (routeView.step > 0) frameStep(routeView.step);
    else mapAdapter.frame(marks);
  } catch (err) {
    status.textContent = `${t("error")}: ${(err as Error).message}`;
  }
}

function onRouteStep(i: number, steps: RouteStop[]): void {
  const key = [steps[0]?.from, ...steps.map((s) => s.to)].join(">");
  if (routeView.key !== key) {
    routeView = { key, locs: null, step: i };
    return;
  }
  if (i === routeView.step) return;
  routeView.step = i;
  frameStep(i);
}

function goHome(): void {
  navigate({ panel: null });
  mapAdapter.focus(null);
  map.resetView();
}

const backRouteBtn = (): HTMLElement =>
  h("button", { type: "button", class: "back-route", onclick: () => myRoute && navigate({ panel: { kind: "path", q: { ...myRoute } } }) }, `↩ ${t("backRoute")}`);

/** «Volver al panorama» always in view with a record or a route open; «Volver a mi ruta» once the reader left one. */
function sideTop(p: Panel): HTMLElement {
  return h("div", { class: "side-top" },
    p ? h("button", { type: "button", class: "back-home", onclick: goHome }, `← ${t("backHome")}`) : null,
    myRoute && p?.kind !== "path" ? backRouteBtn() : null);
}

/** Entry: one question, the search box and three guided doors.  No node counter. */
function homeView(): HTMLElement {
  return h("article", { class: "home", "aria-labelledby": "home-title" },
    h("h2", { id: "home-title", class: "ask", tabindex: -1 }, t("ask")),
    h("p", { class: "ask-lead" }, t("askLead")),
    homeSearch.root,
    h("h3", {}, t("doorsTitle")),
    h("p", { class: "muted small" }, t("doorHint")),
    h("ul", { class: "doors" }, DOORS.map((d) => h("li", {},
      h("button", { type: "button", class: "door", "data-door": d.key, onclick: () => openDoor(d) }, h("span", { class: "door-title" }, t(d.key)), h("span", { class: "door-arrow", "aria-hidden": "true" }, "→"))))));
}

function openDoor(d: (typeof DOORS)[number]): void {
  doorFocus = true;
  navigate({ panel: { kind: "path", q: { ...DEFAULT_PATH, from: { id: d.from, label: d.from }, to: { id: d.to, label: d.to }, step: 0 } } });
}

let sideKey = "";
const wait = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** Replace the panel: a different panel first leaves (150 ms), then the new one enters (200 ms). */
async function setSide(key: string, signal: AbortSignal, ...nodes: (Node | null)[]): Promise<boolean> {
  if (key !== sideKey && sideKey && side.childElementCount) {
    side.classList.remove("entering");
    side.classList.add("leaving");
    await wait((prefersReducedMotion() ? REDUCED_MOTION : MOTION).panelOut);
    side.classList.remove("leaving");
    if (signal.aborted) return false;
  }
  const entering = key !== sideKey;
  sideKey = key;
  side.replaceChildren(...nodes.filter((n): n is Node => !!n));
  if (entering) {
    side.classList.remove("entering");
    void side.offsetWidth; // restart the entry animation
    side.classList.add("entering");
  }
  return true;
}

const errorNote = (err: unknown): HTMLElement => h("p", { class: "notice error", role: "alert" }, `${t("error")}: ${(err as Error).message}`);

async function renderPanel(): Promise<void> {
  panelCtrl?.abort();
  panelCtrl = new AbortController();
  const signal = panelCtrl.signal;
  const p = state.panel;
  side.hidden = false;
  mainEl.classList.add("with-side");
  mainEl.classList.toggle("home", !p);
  if (p?.kind !== "path") map.setPath(null);
  const top = sideTop(p);
  if (!p) {
    await setSide("home", signal, top.childElementCount ? top : null, homeView());
    document.title = t("title");
    return;
  }
  const key = `${p.kind}:${"id" in p ? p.id : ""}`;
  if (p.kind === "entity") {
    if (!(await setSide(key, signal, top, h("p", { class: "muted" }, t("loading"))))) return;
    try {
      let r = await api.entity(p.id, state.lang, signal);
      const extract = (id: string) => api.entity(id, state.lang, signal, { limit: 1 });
      const claimsFrom = (offset: number) => api.entity(p.id, state.lang, signal, { offset, limit: 1000 });
      const show = (): void => side.replaceChildren(top, renderEntity(r, { ...entityActions, loadMore, extract, claimsFrom }));
      // Next page of claims, appended in place; focus stays on the button (or the panel) and the scroll is kept.
      const loadMore = async (): Promise<void> => {
        const btn = side.querySelector<HTMLButtonElement>(".load-more-btn");
        if (btn) btn.disabled = true;
        try {
          const next = await api.entity(p.id, state.lang, signal, { offset: r.claims.length });
          const scroll = side.scrollTop, mainTop = mainEl.scrollTop;
          r = mergeEntityPage(r, next);
          show();
          side.scrollTop = scroll;
          mainEl.scrollTop = mainTop;
          (side.querySelector<HTMLElement>(".load-more-btn") ?? side).focus({ preventScroll: true });
        } catch (err) {
          if ((err as Error).name === "AbortError") return;
          if (btn) btn.disabled = false;
          side.querySelector(".load-more")?.append(errorNote(err));
        }
      };
      show();
      (side.querySelector("#entity-title") as HTMLElement | null)?.focus();
      document.title = `${r.label} · Atlas`;
      void locateAndSelect(r.canonical_id, false, r.label, neighborIds(r.claims, r.canonical_id));
    } catch (err) {
      if ((err as Error).name === "AbortError") return;
      side.replaceChildren(top, errorNote(err));
    }
  } else if (p.kind === "path") {
    const focusResult = doorFocus;
    doorFocus = false;
    if (p.q.from && p.q.to) myRoute = { ...p.q };
    await setSide(key, signal, top, pathView(p.q, {
      open: (id) => navigate({ panel: { kind: "entity", id } }),
      openConcept: (id) => openIdea(id, "concept"),
      showPath: (steps, historical) => void showPath(steps, historical),
      changed: (q) => {
        state.panel = { kind: "path", q };
        if (q.from && q.to) myRoute = { ...q };
        history.replaceState(null, "", encode(state));
      },
      step: onRouteStep,
      focusResult,
    }));
  } else if (p.kind === "compare") {
    const [a, b] = compareSet.map((it) => map.region(it));
    const between = a && b ? map.edgesBetween(a.item, b.item) : new Map<string, number>();
    await setSide(key, signal, top, renderCompare(a ?? null, b ?? null, between, entityActions.open, (r) => void setRegionFocus(r.central[0] ?? null, r.item.label)));
  } else if (p.kind === "concept") {
    if (!(await setSide(key, signal, top, h("p", { class: "muted" }, t("loading"))))) return;
    try {
      const r = await api.concept(p.id, state.lang, p.offset, 50, signal);
      side.replaceChildren(top, renderConcept(r, conceptActions(p.offset)));
      (side.querySelector("#entity-title") as HTMLElement | null)?.focus();
      document.title = `${r.concept.label} · Atlas`;
      void locateAndSelect(r.concept.id, false, r.concept.label, [...r.edges_out, ...r.edges_in].sort((a, b) => b.total - a.total).map((e) => e.concept.id));
    } catch (err) {
      if ((err as Error).name === "AbortError") return;
      side.replaceChildren(top, errorNote(err));
    }
  } else if (p.kind === "region") {
    const r = selectedRegion?.item.id === p.id ? selectedRegion : (() => {
      const it = map.findItem(p.id);
      return it ? map.region(it) : null;
    })();
    await setSide(key, signal, top, r
      ? h("div", {},
        renderCompare(r, null, new Map(), entityActions.open, (x) => void setRegionFocus(x.central[0] ?? null, x.item.label)),
        h("button", { type: "button", onclick: () => addCompare(r.item) }, t("compareWith")))
      : h("p", { class: "muted" }, isExplore() ? t("region") : p.id));
  }
}

// ---- text view -------------------------------------------------------------------

let textTimer = 0;
function scheduleText(): void {
  if (!state.text) return;
  clearTimeout(textTimer);
  textTimer = window.setTimeout(renderText, 250);
}

/** Show or hide the concept index (button and T): focus goes to its heading, and back on close. */
function toggleText(): void {
  const on = !state.text;
  navigate({ text: on });
  if (on) {
    mainEl.scrollTop = 0;
    textFocus.open(textTitle);
  } else textFocus.close(map.canvas);
}

function renderText(): void {
  textPane.hidden = !state.text;
  mainEl.classList.toggle("with-text", state.text);
  if (!state.text) return;
  textTitle.textContent = t("textIndex");
  const items = map.visibleItems();
  // The list is rebuilt as the view changes: a reader tabbing through it keeps their place.
  const buttons = (): HTMLElement[] => [...textBody.querySelectorAll<HTMLElement>("button")];
  const at = buttons().indexOf(document.activeElement as HTMLElement);
  textBody.replaceChildren(renderTextView(items.filter((it) => !isExplore() || map.isLettered(it)), map.hiddenEdges(), {
    open: (it) => navigate({ panel: { kind: "entity", id: it.id } }),
    openConcept: (it) => openIdea(it.id, "concept"),
    zoomInto: (it) => {
      if (it.kind === "concept") {
        void flyToConcept(it.id, !isExplore(), it.size);
        openIdea(it.id, "concept");
        return;
      }
      map.flyTo(it.x, it.y, map.camera.scale * 3);
      selectedRegion = map.region(it);
      navigate({ panel: { kind: "region", id: it.id } });
    },
    compare: (it) => addCompare(it),
  }, map.drawnEdges(), (it) => map.regionName(it)));
  if (at >= 0) (buttons()[at] ?? textTitle).focus({ preventScroll: true });
}

// ---- chrome ---------------------------------------------------------------------

function renderChrome(): void {
  setLang(state.lang);
  setMode(state.mode);
  mapAdapter.setMode(state.mode);
  const explore = state.mode === "explore";
  titleEl.textContent = t("title");
  skipLink.textContent = state.panel ? t("skipPanel") : t("skipDoors");
  search.relabel(t("search"), t("searchPlaceholder"));
  homeSearch.relabel(t("ask"), t("searchPlaceholder"));
  layerLabel.textContent = t("layer");
  layerSelect.replaceChildren(...PROJECTIONS.map((p) => h("option", { value: p, selected: p === state.layer }, `${t("layer")}: ${projectionLabel(p)}`)));
  // The layer is a technical choice; the explore mode reads the structure layer.
  layerSelect.hidden = explore;
  layerSep.hidden = explore;
  modeSwitch.setAttribute("aria-label", t("modeLabel"));
  modeExplore.textContent = t("modeExplore");
  modeTech.textContent = t("modeTechnical");
  modeExplore.setAttribute("aria-pressed", String(explore));
  modeTech.setAttribute("aria-pressed", String(!explore));
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
  legendSummary.textContent = t("legendShort");
  legendBody.setAttribute("aria-label", t("legendTitle"));
  legendBody.replaceChildren(legendList(false), h("p", { class: "legend-dots" }, t("legendDotsPlain")));
  // No node counter in the explore mode.
  if (nodeCount !== null) status.textContent = explore ? "" : t("nodesCount", { n: nodeCount.toLocaleString(state.lang) });
  map.canvas.setAttribute("aria-label", `${t("title")} — ${t("help")}`);
  textTitle.textContent = t("textIndex");
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
    toggleText();
  } else if (e.key === "Escape") {
    if (regionFocus) void setRegionFocus(null, null);
    else if (state.panel) goHome();
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
    status.title = map.rendererName;
    if (layoutVersion) {
      const root = await api.tile(state.layer, 0, 0, 0, state.lang, layoutVersion);
      maxZoom = root.maxZoom;
      map.setHome(contentBounds(root.px, root.py, root.nNodes));
    } else {
      mapHost.append(h("p", { class: "notice map-notice" }, t("noLayout")));
      textBody.append(legend());
    }
  } catch (err) {
    status.textContent = `${t("error")}: ${(err as Error).message}`;
  }
  render();
}

void boot();
