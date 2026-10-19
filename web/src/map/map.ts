// The multiscale map: camera + tile streaming + WebGL2 layers + a 2-D overlay for
// labels, focus, terra incognita and the path.  Zoom is continuous; when the tile level
// changes, the new level fades in over the old one, which fades out once the
// viewport is covered, so regions dissolve into their members instead of popping.
//
// Round 4 "printed atlas" (docs/design/r4/guia-skills-diseno.md): two modes ("explore",
// the default, and "technical"); areas lettered as continents, concepts as cities,
// declarations only in technical.  Gold means validated and nothing else: in explore the
// background edges are one faint neutral, and gold appears only on validated edges of the
// concept in focus or of the active route.  Focus = ink + size + ring + the paper veil.
import { centralOf, KIND_CLUSTER, KIND_CONCEPT, type Tile } from "../tile.ts";
import { Camera, coveringTiles, type View } from "./camera.ts";
import { Renderer, type Frame, type Layer, type Theme } from "./gl.ts";
import { EVIDENCE_STATES, typeName } from "../vocab.ts";
import { t, typeLabel } from "../i18n.ts";
import {
  cssFont, displayLabel, labelBudget, labelHeight, labelTier, placeLabels, STATEMENT_MIN_ZOOM, TIER_RANK, tierStyle, veiledStyle,
  type LabelCandidate, type LabelTier, type MapMode, type PlacedLabel,
} from "./labels.ts";
import { edgeAlpha, edgeStateColors, pointPalette, rankNeighbors, routeHopMark, type NeighborCandidate } from "./style.ts";
import { itemRadius, TileStore, type Entry } from "./tiles.ts";
import { EVIDENCE, FOCUS, FONT, MAP_LABELS, PALETTE, UNKNOWN_HATCH, VEIL, type EvidenceMark, type MapLabelStyle, type Palette, type ThemeName } from "../tokens.ts";

const FADE_IN_MS = 220;
const FADE_OUT_MS = 260;
const MAX_LABEL_CANDIDATES = 5000;
const MAX_NEIGHBORS = 7;
const KEY_PAN_PX = 80;
const KEY_ZOOM = 1.5;
const LABEL_CHARS = 34;
const MARK_FONT = `700 12px ${FONT.ui}`;
/** Benchmark labels: the city style, same placement and paint code as the real map. */
const BENCH_STYLE = MAP_LABELS.concept;
const BENCH_MAX_LABELS = 220; // as before round 4, so frame times stay comparable

export type { MapMode } from "./labels.ts";

export interface MapItem {
  id: string;
  label: string;
  kind: "entity" | "cluster" | "concept";
  type: number;
  x: number;
  y: number;
  size: number;
  priority: number;
  community: string;
  tile: Tile;
  index: number;
}

export interface RegionInfo {
  item: MapItem;
  members: number;
  hidden: number;
  relations: [number, number][]; // (relation code, count)
  central: string[];
}

export interface PathMark {
  id: string;
  label: string;
  x: number;
  y: number;
  /** Evidence state of the hop that arrives at this stop (stop 0 has none).  Each hop is
   *  drawn in its state's stroke (gold only when validated); without one, as a candidate. */
  state?: string;
}

/** What `focus()` lights up.  Position and label are used while the item's tile is not
 *  loaded; `neighbors` (ids, best first) replaces the ranking from the tiles' edges. */
export interface FocusOptions {
  x?: number;
  y?: number;
  label?: string;
  neighbors?: readonly string[];
}

export interface MapEvents {
  onSelect(item: MapItem, shift: boolean): void;
  onViewChange(): void;
}

export interface FpsReport {
  frames: number;
  seconds: number;
  fps: number;
  p50_ms: number;
  p95_ms: number;
  p99_ms: number;
  cpu_p50_ms: number;
  cpu_p95_ms: number;
  points: number;
  edges: number;
  labels_p50: number;
  renderer: string;
  width: number;
  height: number;
  dpr: number;
}

function itemOf(t: Tile, i: number): MapItem {
  return {
    id: t.strings[t.id[i]],
    label: t.strings[t.label[i]],
    kind: t.kind[i] === KIND_CLUSTER ? "cluster" : t.kind[i] === KIND_CONCEPT ? "concept" : "entity",
    type: t.type[i],
    x: t.px[i],
    y: t.py[i],
    size: t.size[i],
    priority: t.priority[i],
    community: t.strings[t.community[i]],
    tile: t,
    index: i,
  };
}

/** Spoken kind of an item (region, concept, or the entity's type). */
export function kindLabel(it: MapItem): string {
  return it.kind === "cluster" ? t("region") : it.kind === "concept" ? t("conceptMap") : typeLabel(typeName(it.type));
}

const short = (s: string): string => (s.length > LABEL_CHARS ? `${s.slice(0, LABEL_CHARS - 1)}…` : s);
const tierOf = (it: MapItem, mode: MapMode): LabelTier | null =>
  labelTier({ kind: it.kind, type: typeName(it.type), id: it.id, label: it.label }, mode);

/** Short cache key per lettering style (widths are cached per style + text). */
const STYLE_KEY = new Map<MapLabelStyle, string>(Object.entries(MAP_LABELS).map(([k, v]) => [v, k]));
const styleKey = (s: MapLabelStyle): string => {
  let k = STYLE_KEY.get(s);
  if (!k) STYLE_KEY.set(s, (k = `s${STYLE_KEY.size}`));
  return k;
};

interface FocusEdge {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  state: number;
}

interface FocusState {
  id: string;
  opts: FocusOptions;
  /** Resolved against the loaded tiles (refreshFocus). */
  item: MapItem | null;
  neighbors: MapItem[];
  edges: FocusEdge[];
  gen: number;
  z: number;
  mode: MapMode;
}

/** A label to paint: where (placed box), what, in which lettering and ink. */
interface Lettering {
  text: string;
  style: MapLabelStyle;
  color: string;
}

export class MapView {
  readonly camera = new Camera();
  readonly canvas: HTMLCanvasElement;
  private readonly overlay: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly renderer: Renderer;
  private readonly store: TileStore;
  private readonly events: MapEvents;
  private maxZoom = 0;
  private curZ = 0;
  private prevZ: number | null = null;
  private coveredAt = 0;
  private items: MapItem[] = [];
  private tiers: (LabelTier | null)[] = [];
  private byId = new Map<string, MapItem>();
  private unknown = new Set<MapItem>();
  private itemsGen = -1;
  private itemsZ = -1;
  private itemsMode: MapMode = "explore";
  private placed: PlacedLabel[] = [];
  private placedItems: (MapItem | null)[] = [];
  private widths = new Map<string, number>();
  private hover: MapItem | null = null;
  private kbIndex = -1;
  private selected: { x: number; y: number } | null = null;
  private path: PathMark[] | null = null;
  private pathHistorical = false;
  /** Explore names of regions: their most central lettered member (never an article or a person). */
  private regionNames = new Map<MapItem, string>();
  private tierOfItem = new Map<MapItem, LabelTier | null>();
  private points: { lit: Float32Array; veiled: Float32Array } | null = null;
  private focusState: FocusState | null = null;
  private mapMode: MapMode = "explore";
  private themeName: ThemeName = "light";
  private gpuTheme: Theme | null = null;
  private edgeColors: Float32Array | null = null;
  private pal: Palette = PALETTE.light;
  private hatch: CanvasPattern | null = null;
  private compare: MapItem[] = [];
  private dirty = true;
  private raf = 0;
  private dark = false;
  private benchLayer: Layer | null = null;
  private lastFrameMs = 0;
  private wanted = new Set<string>();
  private drag: { x: number; y: number; moved: boolean } | null = null;
  private pinch = new Map<number, [number, number]>();
  enabled = false;

  constructor(host: HTMLElement, events: MapEvents) {
    this.events = events;
    this.canvas = document.createElement("canvas");
    this.canvas.className = "map-gl";
    this.canvas.tabIndex = 0;
    this.canvas.setAttribute("role", "application");
    this.overlay = document.createElement("canvas");
    this.overlay.className = "map-overlay";
    this.overlay.setAttribute("aria-hidden", "true");
    host.append(this.canvas, this.overlay);
    this.ctx = this.overlay.getContext("2d")!;
    this.renderer = new Renderer(this.canvas);
    this.store = new TileStore(this.renderer, () => this.invalidate());
    new ResizeObserver(() => this.resize(host)).observe(host);
    this.resize(host);
    this.bindPointer();
  }

  get rendererName(): string {
    return this.renderer.renderer;
  }

  get lastError(): string | null {
    return this.store.lastError;
  }

  setParams(layer: string, lang: string, version: string, maxZoom: number): void {
    this.maxZoom = maxZoom;
    this.enabled = true;
    this.store.setParams({ layer, lang, version });
    this.itemsGen = -1;
    this.invalidate();
  }

  setTheme(theme: Theme, dark: boolean): void {
    this.dark = dark;
    this.themeName = dark ? "dark" : "light";
    this.gpuTheme = theme;
    this.renderer.setTheme(theme);
    // Token values, overridden by the CSS ones when the page sets them (high contrast);
    // read once per theme switch, not per frame.
    const cs = getComputedStyle(document.documentElement);
    const base = PALETTE[this.themeName];
    const pal = { ...base };
    for (const k of Object.keys(base) as (keyof Palette)[]) pal[k] = cs.getPropertyValue(`--color-${k}`).trim() || base[k];
    this.pal = pal;
    this.hatch = null;
    this.edgeColors = edgeStateColors(this.themeName, this.mapMode, theme.stateColor);
    this.points = { lit: pointPalette(this.themeName, false), veiled: pointPalette(this.themeName, true) };
    this.invalidate();
  }

  /** "explore" (default): no library names, ids or declaration edges; neutral background
   *  edges.  "technical": every label and the evidence colors of every edge. */
  setMode(mode: MapMode): void {
    if (mode === this.mapMode) return;
    this.mapMode = mode;
    if (this.gpuTheme) this.edgeColors = edgeStateColors(this.themeName, mode, this.gpuTheme.stateColor);
    this.itemsGen = -1;
    this.invalidate();
  }

  get mode(): MapMode {
    return this.mapMode;
  }

  setFocus(communities: string[] | null): void {
    this.store.setFocus(communities ? new Set(communities) : null);
    this.invalidate();
  }

  /** One protagonist: the item `id` (focus lettering, dot and ring) and its most important
   *  neighbors (at most 7, mathematical relations before citations) stay lit; the rest of
   *  the map goes under the paper veil with veiled ink.  null clears it. */
  focus(id: string | null, opts: FocusOptions = {}): void {
    this.focusState = id ? { id, opts, item: null, neighbors: [], edges: [], gen: -1, z: -1, mode: this.mapMode } : null;
    this.invalidate();
  }

  /** Id of the item in focus, null without one. */
  get focusedId(): string | null {
    return this.focusState?.id ?? null;
  }

  /** Neighbors drawn lit around the focus (resolved against the loaded tiles). */
  focusNeighbors(): MapItem[] {
    return this.focusState?.neighbors ?? [];
  }

  setSelected(pos: { x: number; y: number } | null): void {
    this.selected = pos;
    this.invalidate();
  }

  /** `historical`: a historical route, drawn in the long ink dash of «Histórica».  Any other
   *  route (documentary, exploration, candidate) is drawn hop by hop in its evidence states. */
  setPath(marks: PathMark[] | null, historical = false): void {
    this.path = marks;
    this.pathHistorical = historical;
    if (marks && marks.length) {
      const xs = marks.map((m) => m.x), ys = marks.map((m) => m.y);
      this.camera.fit(Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys));
    }
    this.invalidate();
  }

  setCompare(items: MapItem[]): void {
    this.compare = items;
    this.invalidate();
  }

  /** Fly to a point (camera flight of MOTION.camera; a jump under reduced motion). */
  flyTo(x: number, y: number, scale?: number): void {
    this.camera.flyTo({ cx: x, cy: y, scale: scale ?? Math.max(this.camera.scale, 384 * 2 ** Math.min(this.maxZoom, 6)) });
    this.invalidate();
  }

  /** Fly so the world rectangle [x0, y0, x1, y1] fills most of the view (a region, a door). */
  flyToBounds(x0: number, y0: number, x1: number, y1: number): void {
    const c = this.camera;
    const w = Math.max(x1 - x0, 1e-6), h = Math.max(y1 - y0, 1e-6);
    c.flyTo({ cx: (x0 + x1) / 2, cy: (y0 + y1) / 2, scale: Math.min(c.width / w, c.height / h) * 0.85 });
    this.invalidate();
  }

  setView(v: View, animate = false): void {
    if (animate) this.camera.flyTo(v);
    else {
      this.camera.followHome = false;
      this.camera.set(v);
    }
    this.invalidate();
  }

  resetView(animate = true): void {
    this.camera.goHome(animate ? undefined : 0);
    this.invalidate();
  }

  /** Frame the content (`contentBounds` of the root tile) instead of the whole unit square. */
  setHome(bounds: [number, number, number, number] | null): void {
    this.camera.home = bounds;
    this.camera.goHome(0);
    this.invalidate();
  }

  invalidate(): void {
    this.dirty = true;
    if (!this.raf) this.raf = requestAnimationFrame((t) => this.frame(t));
  }

  private resize(host: HTMLElement): void {
    const r = host.getBoundingClientRect();
    this.camera.resize(r.width, r.height);
    const dpr = devicePixelRatio || 1;
    this.overlay.width = Math.round(r.width * dpr);
    this.overlay.height = Math.round(r.height * dpr);
    this.overlay.style.width = `${r.width}px`;
    this.overlay.style.height = `${r.height}px`;
    this.canvas.style.width = `${r.width}px`;
    this.canvas.style.height = `${r.height}px`;
    this.invalidate();
  }

  // ---- frame loop -------------------------------------------------------------

  private frame(now: number): void {
    this.raf = 0;
    const animating = this.camera.step(now);
    const t0 = performance.now();
    let busy = animating;
    if (this.benchLayer) {
      this.drawBench();
      busy = true;
    } else if (this.enabled) {
      busy = this.drawTiles(now) || busy;
    }
    this.lastFrameMs = performance.now() - t0;
    if (this.dirty || busy) {
      this.dirty = false;
      this.events.onViewChange();
    }
    if (busy) this.raf = requestAnimationFrame((t) => this.frame(t));
  }

  private frameInfo(): Frame {
    const c = this.camera;
    return { cx: c.cx, cy: c.cy, scale: c.scale, width: c.width, height: c.height, dpr: devicePixelRatio || 1 };
  }

  /** Draw the tile layers; true while something is still fading or loading. */
  private drawTiles(now: number): boolean {
    const z = this.camera.tileZoom(this.maxZoom);
    if (z !== this.curZ) {
      this.prevZ = this.curZ;
      this.curZ = z;
      this.coveredAt = 0;
    }
    const bounds = this.camera.bounds();
    const need = coveringTiles(z, bounds);
    const keep = new Set<string>();
    if (this.prevZ !== null) for (const [x, y] of coveringTiles(this.prevZ, bounds)) keep.add(`${this.prevZ}/${x}/${y}`);
    // The viewport first (nearest to the center first), then a one-tile margin.
    this.store.want(z, [...need, ...coveringTiles(z, bounds, 1)], keep);
    this.wanted = new Set(need.map(([x, y]) => `${z}/${x}/${y}`));

    const cur: [Layer, number][] = [];
    let covered = true;
    let fading = false;
    for (const [x, y] of need) {
      const e = this.store.get(z, x, y);
      if (!e || e.state === "loading") covered = false;
      if (e?.state !== "ready" || !e.gpu) continue;
      const a = Math.min(1, (now - e.readyAt) / FADE_IN_MS);
      fading ||= a < 1;
      cur.push([e.gpu, a]);
    }
    if (covered && !fading && this.coveredAt === 0) this.coveredAt = now;
    const prev: [Layer, number][] = [];
    if (this.prevZ !== null) {
      const a = this.coveredAt === 0 ? 1 : 1 - (now - this.coveredAt) / FADE_OUT_MS;
      if (a <= 0) this.prevZ = null;
      else {
        fading = true;
        for (const [x, y] of coveringTiles(this.prevZ, bounds)) {
          const e = this.store.get(this.prevZ, x, y);
          if (e?.state === "ready" && e.gpu) prev.push([e.gpu, a]);
        }
      }
    }
    this.refreshItems(z);
    this.refreshFocus(z);
    const veiled = this.isVeiled();
    const f = this.frameInfo();
    this.renderer.begin(f);
    let nEdges = 0;
    for (const [l] of cur) nEdges += l.nEdges;
    const explore = this.mapMode === "explore";
    this.renderer.drawEdges([...prev, ...cur], f, edgeAlpha(this.mapMode, this.dark, nEdges) * (veiled ? VEIL.edgeFactor : 1), {
      stateColor: this.edgeColors ?? undefined,
      hideLibrary: explore,
    });
    this.renderer.drawPoints([...prev, ...cur], f, this.points ? this.points[veiled ? "veiled" : "lit"] : undefined);
    this.drawOverlay(z, veiled);
    const loading = need.some(([x, y]) => this.store.get(z, x, y)?.state === "loading");
    return fading || loading;
  }

  /** Items of the ready tiles at the current zoom, in label priority order, with their tier
   *  and the regions known to have no extracted relation (terra incognita). */
  private refreshItems(z: number): void {
    if (this.itemsGen === this.store.generation && this.itemsZ === z && this.itemsMode === this.mapMode) return;
    this.itemsGen = this.store.generation;
    this.itemsZ = z;
    this.itemsMode = this.mapMode;
    const out: { it: MapItem; tier: LabelTier | null; late: boolean }[] = [];
    const unknown = new Set<MapItem>();
    const explore = this.mapMode === "explore";
    for (const e of this.store.readyAt(z)) {
      const tl = e.tile!;
      const dropped = tl.hiddenCount.some((n) => n > 0);
      const touched = new Uint8Array(tl.nItems);
      for (let k = 0; k < tl.nEdges; k++) touched[tl.edgeSrc[k]] = touched[tl.edgeDst[k]] = 1;
      for (let a = 0; a < tl.nAggs; a++) if (tl.aggRelOff[a + 1] > tl.aggRelOff[a]) touched[tl.aggItem[a]] = 1;
      for (let i = 0; i < tl.nNodes; i++) {
        const it = itemOf(tl, i);
        // Explore letters an area or region without a label in the active language only
        // after those that have one (C4-13): it sorts last within its tier.
        out.push({ it, tier: tierOf(it, this.mapMode), late: explore && !!e.untranslated?.[i] });
        // A region is unknown only when its tile kept every edge (nothing was dropped by the
        // tile budget) and none touches it, and its members have no relation among them.
        // Why none shows at z0-z2 (C4-15, measured 2026-10-19 on layout 649b68d3): the general
        // layer has no regions at all (0 clusters in its 21 tiles; areas and concepts only) and
        // 20 of those 21 tiles drop edges over budget, so from tile data no item there can be
        // said to have no relation.  Saying it truthfully needs a per-item "no extracted
        // relation" flag from the registry in the tile (server side); until then the rule stays.
        if (!dropped && tl.kind[i] === KIND_CLUSTER && !touched[i]) unknown.add(it);
      }
    }
    const byId = new Map<string, MapItem>();
    for (const x of out) if (!byId.has(x.it.id)) byId.set(x.it.id, x.it);
    // Explore names a region after its most central member that explore letters itself
    // (not a person, an article or a declaration); without one it stays an unnamed disc.
    const regionNames = new Map<MapItem, string>();
    if (explore) {
      for (const x of out) {
        if (x.it.kind !== "cluster" || !x.tier) continue;
        const m = centralOf(x.it.tile, x.it.index).map((id) => byId.get(id)).find((c) => c && c.kind !== "cluster" && tierOf(c, this.mapMode));
        if (m) regionNames.set(x.it, m.label);
        else x.tier = null;
      }
    }
    // Label order: areas by size, then subareas by size, concepts and declarations by priority.
    const rank = (x: { tier: LabelTier | null; late: boolean }): number => (x.tier ? TIER_RANK[x.tier] + (x.late ? 0.5 : 0) : 9);
    out.sort((a, b) => {
      const d = rank(a) - rank(b);
      if (d) return d;
      return a.it.kind === "cluster" || a.tier === "area" ? b.it.size - a.it.size || a.it.priority - b.it.priority : a.it.priority - b.it.priority;
    });
    this.items = out.map((x) => x.it);
    this.tiers = out.map((x) => x.tier);
    this.tierOfItem = new Map(out.map((x) => [x.it, x.tier]));
    this.regionNames = regionNames;
    this.byId = byId;
    this.unknown = unknown;
    this.kbIndex = -1;
  }

  /** Resolve the focus against the loaded tiles: its item, neighbors and connecting edges. */
  private refreshFocus(z: number): void {
    const f = this.focusState;
    if (!f || (f.gen === this.itemsGen && f.z === z && f.mode === this.mapMode)) return;
    f.gen = this.itemsGen;
    f.z = z;
    f.mode = this.mapMode;
    f.item = this.byId.get(f.id) ?? null;
    const cands: NeighborCandidate[] = [];
    const ends = new Map<string, MapItem>();
    const links: { other: string; edge: FocusEdge }[] = [];
    for (const e of this.store.readyAt(z)) {
      const tl = e.tile!;
      for (let k = 0; k < tl.nEdges; k++) {
        const a = tl.edgeSrc[k], b = tl.edgeDst[k];
        const ia = tl.strings[tl.id[a]], ib = tl.strings[tl.id[b]];
        if (ia !== f.id && ib !== f.id) continue;
        const [self, other, otherId] = ia === f.id ? [a, b, ib] : [b, a, ia];
        if (otherId === f.id) continue;
        if (!ends.has(otherId)) ends.set(otherId, this.byId.get(otherId) ?? itemOf(tl, other));
        if (!f.item) f.item = itemOf(tl, self);
        cands.push({ id: otherId, rel: tl.edgeRel[k], count: tl.edgeCount[k] });
        links.push({ other: otherId, edge: { x0: tl.px[self], y0: tl.py[self], x1: tl.px[other], y1: tl.py[other], state: tl.edgeState[k] } });
      }
    }
    const lettered = (it: MapItem | undefined): it is MapItem => !!it && tierOf(it, this.mapMode) !== null;
    let ids: string[];
    if (f.opts.neighbors) ids = f.opts.neighbors.filter((id) => id !== f.id && lettered(this.byId.get(id) ?? ends.get(id))).slice(0, MAX_NEIGHBORS);
    else ids = rankNeighbors(cands.filter((c) => lettered(ends.get(c.id))), MAX_NEIGHBORS);
    f.neighbors = ids.map((id) => (this.byId.get(id) ?? ends.get(id))!);
    const chosen = new Set(ids);
    // Both endpoint tiles carry an edge: keep one copy per (neighbor, state).
    const seen = new Set<string>();
    f.edges = links.filter((l) => {
      const key = `${l.other}:${l.edge.state}`;
      if (!chosen.has(l.other) || seen.has(key)) return false;
      seen.add(key);
      return true;
    }).map((l) => l.edge);
  }

  private radiusOf(it: MapItem): number {
    return it.tile.kind ? itemRadius(it.tile, it.index) : 3;
  }

  /** Set the canvas lettering state of a style (font, small caps, tracking). */
  private letter(s: MapLabelStyle): void {
    const ctx = this.ctx;
    ctx.font = cssFont(s, s.family === "serif" ? FONT.serif : FONT.ui);
    ctx.fontVariantCaps = s.smallCaps ? "all-small-caps" : "normal";
    ctx.letterSpacing = s.trackingEm ? `${(s.trackingEm * s.size).toFixed(2)}px` : "0px";
  }

  private textWidth(s: string, style: MapLabelStyle): number {
    const key = `${styleKey(style)}|${s}`;
    let w = this.widths.get(key);
    if (w === undefined) {
      if (this.widths.size > 50_000) this.widths.clear();
      this.letter(style);
      w = Math.ceil(this.ctx.measureText(s).width);
      this.widths.set(key, w);
    }
    return w;
  }

  /** A focus or a route is on: the rest of the map goes to veiled ink. */
  private isVeiled(): boolean {
    const fs = this.focusState;
    return !!this.path?.length || !!(fs && (fs.item || fs.opts.x !== undefined));
  }

  private drawOverlay(z: number, veiled: boolean): void {
    const ctx = this.ctx;
    const dpr = devicePixelRatio || 1;
    const { width, height } = this.camera;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const pal = this.pal;
    const fs = this.focusState;
    const routed = !!this.path?.length;

    // Pinned labels first (focus, its neighbors, route stops), then the hierarchy by budget.
    const cands: LabelCandidate[] = [];
    const letters: Lettering[] = [];
    const owners: (MapItem | null)[] = [];
    const pinned = new Set<string>();
    const pin = (id: string, x: number, y: number, r: number, label: string, style: MapLabelStyle, owner: MapItem | null): void => {
      if (pinned.has(id)) return;
      pinned.add(id);
      const [sx, sy] = this.camera.toScreen(x, y);
      const text = short(displayLabel(label));
      cands.push({ key: letters.length, x: sx, y: sy, r, w: this.textWidth(text, style), h: labelHeight(style) });
      letters.push({ text, style, color: pal["text-primary"] });
      owners.push(owner);
    };
    const focusPos = fs && (fs.item ? { x: fs.item.x, y: fs.item.y } : fs.opts.x !== undefined && fs.opts.y !== undefined ? { x: fs.opts.x, y: fs.opts.y } : null);
    if (fs && focusPos) {
      pin(fs.id, focusPos.x, focusPos.y, FOCUS.focusDot + FOCUS.ring + 2, fs.item?.label ?? fs.opts.label ?? "", MAP_LABELS.focus, fs.item);
      for (const n of fs.neighbors) pin(n.id, n.x, n.y, Math.max(FOCUS.dot, this.radiusOf(n)) + 1, n.label, MAP_LABELS.neighbor, n);
    }
    if (routed) for (const m of this.path!) pin(m.id, m.x, m.y, 12, m.label, MAP_LABELS.neighbor, this.byId.get(m.id) ?? null);
    const nPinned = cands.length;

    const budget = labelBudget(z, width, height);
    const statements = this.mapMode === "technical" && z >= STATEMENT_MIN_ZOOM;
    for (let i = 0; i < this.items.length && cands.length < MAX_LABEL_CANDIDATES; i++) {
      const tier = this.tiers[i];
      if (!tier) break; // unlabelled items sort last
      if (tier === "statement" && !statements) continue;
      const it = this.items[i];
      if (pinned.has(it.id)) continue;
      const [sx, sy] = this.camera.toScreen(it.x, it.y);
      if (sx < -40 || sy < -20 || sx > width + 40 || sy > height + 20) continue;
      const style = veiled ? veiledStyle(tierStyle(tier, z)) : tierStyle(tier, z);
      const text = short(displayLabel(this.regionNames.get(it) ?? it.label));
      cands.push({
        key: letters.length, x: sx, y: sy, r: this.radiusOf(it), w: this.textWidth(text, style), h: labelHeight(style),
        anchor: tier === "area" ? "center" : undefined, group: tier === "area" ? 0 : undefined,
      });
      letters.push({ text, style, color: pal[style.color] });
      owners.push(it);
    }
    this.placed = placeLabels(cands, width, height, budget.total + nPinned, [budget.areas]);
    this.placedItems = this.placed.map((p) => owners[p.key]);

    // No paper wash: the veil is the lighter ink of points and labels (tokens VEIL), so the
    // contrast drawn is the contrast measured.
    this.paintUnknown();
    if (fs && focusPos) this.paintFocus(fs, focusPos);
    if (routed) this.paintRoute(this.path!);
    this.paintLabels(this.placed, letters);
    this.paintMarks();
  }

  private paintLabels(placed: PlacedLabel[], letters: Lettering[]): void {
    const ctx = this.ctx;
    ctx.textBaseline = "top";
    ctx.textAlign = "start";
    ctx.lineJoin = "round";
    ctx.lineWidth = 3;
    ctx.strokeStyle = this.pal.halo;
    let last: MapLabelStyle | null = null;
    for (const p of placed) {
      const l = letters[p.key];
      if (l.style !== last) {
        this.letter(l.style);
        last = l.style;
      }
      const y = p.y + (p.h - l.style.size) / 2;
      ctx.strokeText(l.text, p.x, y);
      ctx.fillStyle = l.color;
      ctx.fillText(l.text, p.x, y);
    }
    ctx.fontVariantCaps = "normal";
    ctx.letterSpacing = "0px";
  }

  /** Terra incognita: regions known to have no extracted relation, hatched (never "isolated"). */
  private paintUnknown(): void {
    if (!this.unknown.size) return;
    const ctx = this.ctx;
    const pat = this.hatchPattern();
    for (const it of this.unknownInView()) {
      const [sx, sy] = this.camera.toScreen(it.x, it.y);
      const r = this.radiusOf(it) + 4;
      ctx.beginPath();
      ctx.arc(sx, sy, r, 0, Math.PI * 2);
      ctx.fillStyle = this.pal.unknown;
      ctx.fill();
      if (pat) {
        ctx.fillStyle = pat;
        ctx.fill();
      }
      ctx.lineWidth = 1;
      ctx.strokeStyle = this.pal["unknown-hatch"];
      ctx.stroke();
    }
  }

  private hatchPattern(): CanvasPattern | null {
    if (this.hatch) return this.hatch;
    const { spacing, width } = UNKNOWN_HATCH; // 45 degrees: the diagonal of a square tile
    const c = document.createElement("canvas");
    c.width = c.height = spacing;
    const g = c.getContext("2d");
    if (!g) return null;
    g.strokeStyle = this.pal["unknown-hatch"];
    g.lineWidth = width;
    g.beginPath();
    for (const o of [-spacing, 0, spacing]) {
      g.moveTo(o, spacing);
      g.lineTo(o + spacing, 0);
    }
    g.stroke();
    this.hatch = this.ctx.createPattern(c, "repeat");
    return this.hatch;
  }

  /** Dash of an evidence mark in CSS px ([] = solid). */
  private static dash(m: EvidenceMark): number[] {
    return m.dash ? [m.dash * m.duty, m.dash * (1 - m.duty)] : [];
  }

  private paintFocus(fs: FocusState, pos: { x: number; y: number }): void {
    const ctx = this.ctx;
    const pal = this.pal;
    ctx.lineCap = "round";
    for (const e of fs.edges) {
      const m = EVIDENCE[EVIDENCE_STATES[e.state] ?? ""] ?? EVIDENCE.extracted;
      const [x0, y0] = this.camera.toScreen(e.x0, e.y0);
      const [x1, y1] = this.camera.toScreen(e.x1, e.y1);
      ctx.beginPath();
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y1);
      ctx.setLineDash([]);
      ctx.strokeStyle = pal.halo;
      ctx.lineWidth = m.width + 3;
      ctx.stroke();
      ctx.setLineDash(MapView.dash(m));
      ctx.strokeStyle = pal[m.color];
      ctx.lineWidth = m.width;
      ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.lineCap = "butt";
    for (const n of fs.neighbors) this.dot(n.x, n.y, FOCUS.dot, pal["text-primary"]);
    this.dot(pos.x, pos.y, FOCUS.focusDot, pal["text-primary"]);
    this.ring(pos.x, pos.y, FOCUS.focusDot + FOCUS.ring + 1, pal["focus-ring"], FOCUS.ring);
  }

  private dot(x: number, y: number, r: number, color: string): void {
    const ctx = this.ctx;
    const [sx, sy] = this.camera.toScreen(x, y);
    ctx.beginPath();
    ctx.arc(sx, sy, r, 0, Math.PI * 2);
    ctx.fillStyle = color;
    ctx.fill();
    ctx.lineWidth = 1.5;
    ctx.strokeStyle = this.pal.halo;
    ctx.stroke();
  }

  private ring(x: number, y: number, r: number, color: string, width = 2.5, dash: number[] = []): void {
    const ctx = this.ctx;
    const [sx, sy] = this.camera.toScreen(x, y);
    ctx.beginPath();
    ctx.setLineDash(dash);
    ctx.arc(sx, sy, r, 0, Math.PI * 2);
    ctx.strokeStyle = color;
    ctx.lineWidth = width;
    ctx.stroke();
    ctx.setLineDash([]);
  }

  /** The route: one segment per hop in the hop's evidence mark (routeHopMark: gold only when
   *  validated, dotted when candidate or unknown); a historical route in the long ink dash. */
  private paintRoute(path: PathMark[]): void {
    const ctx = this.ctx;
    const pal = this.pal;
    const pts = path.map((m) => this.camera.toScreen(m.x, m.y));
    ctx.lineJoin = "round";
    ctx.lineCap = "round";
    ctx.beginPath();
    pts.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y)));
    ctx.strokeStyle = pal.halo;
    ctx.lineWidth = 7;
    ctx.stroke();
    for (let i = 1; i < pts.length; i++) {
      const m = routeHopMark(path[i].state, this.pathHistorical);
      ctx.beginPath();
      ctx.moveTo(pts[i - 1][0], pts[i - 1][1]);
      ctx.lineTo(pts[i][0], pts[i][1]);
      ctx.setLineDash(MapView.dash(m));
      ctx.strokeStyle = pal[m.color];
      ctx.lineWidth = Math.max(2, m.width + 0.5);
      ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.lineCap = "butt";
    ctx.font = MARK_FONT;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    pts.forEach(([sx, sy], i) => {
      ctx.beginPath();
      ctx.arc(sx, sy, 10, 0, Math.PI * 2);
      ctx.fillStyle = pal["text-primary"];
      ctx.fill();
      ctx.lineWidth = 2.5;
      ctx.strokeStyle = pal.halo;
      ctx.stroke();
      ctx.fillStyle = pal["on-ink"];
      ctx.fillText(String(i + 1), sx, sy + 0.5);
    });
    ctx.textAlign = "start";
  }

  private paintMarks(): void {
    const pal = this.pal;
    this.compare.forEach((it, i) => this.ring(it.x, it.y, this.radiusOf(it) + 6, pal["text-primary"], 3, i === 0 ? [] : [6, 4]));
    if (this.selected && !this.focusState) this.ring(this.selected.x, this.selected.y, 11, pal["focus-ring"], FOCUS.ring + 1);
    if (this.hover) this.ring(this.hover.x, this.hover.y, this.radiusOf(this.hover) + 4, pal["text-secondary"], 1.5);
    const kb = this.placedItems[this.kbIndex];
    if (kb) {
      this.ring(kb.x, kb.y, this.radiusOf(kb) + 7, pal["focus-ring"], 2, [3, 3]);
      this.canvas.setAttribute("aria-label", `${kb.label} (${kindLabel(kb)})`);
    }
  }

  // ---- queries for the panels and the text view --------------------------------

  /** Items in view, regions first, in label priority order (what the text view lists). */
  visibleItems(limit = 400): MapItem[] {
    const [x0, y0, x1, y1] = this.camera.bounds();
    const out: MapItem[] = [];
    for (const it of this.items) {
      if (it.x < x0 || it.x > x1 || it.y < y0 || it.y > y1) continue;
      out.push(it);
      if (out.length >= limit) break;
    }
    return out;
  }

  /** Regions on screen known to have no extracted relation (the legend names them
   *  "desconocida"); empty when none is in view. */
  unknownInView(): MapItem[] {
    const [x0, y0, x1, y1] = this.camera.bounds();
    return [...this.unknown].filter((it) => it.x >= x0 && it.x <= x1 && it.y >= y0 && it.y <= y1);
  }

  /** Whether the current mode would letter this item on the map (explore hides ids,
   *  library names and declarations); for lists that should match the map. */
  isLettered(it: MapItem): boolean {
    const t = this.tierOfItem.get(it);
    return (t === undefined ? tierOf(it, this.mapMode) : t) !== null;
  }

  /** Name the map gives a region in the current mode: in explore its most central lettered
   *  member (never an article title or a person), null when none is loaded; in technical
   *  its own label.  For lists that should match the map (the concept index). */
  regionName(it: MapItem): string | null {
    if (this.mapMode === "technical") return it.label;
    return this.regionNames.get(it) ?? null;
  }

  /** Edges counted but not drawn in the tiles in view, per relation code. */
  hiddenEdges(): Map<number, number> {
    const out = new Map<number, number>();
    for (const e of this.store.readyAt(this.curZ)) {
      if (!this.wanted.has(e.key)) continue;
      const t = e.tile!;
      for (let k = 0; k < t.hiddenRel.length; k++) out.set(t.hiddenRel[k], (out.get(t.hiddenRel[k]) ?? 0) + t.hiddenCount[k]);
    }
    return out;
  }

  /** Edges drawn by the tiles in view (0 does not mean "no relation": see the legend). */
  drawnEdges(): number {
    let n = 0;
    for (const e of this.store.readyAt(this.curZ)) if (this.wanted.has(e.key)) n += e.tile!.nEdges;
    return n;
  }

  region(it: MapItem): RegionInfo {
    const t = it.tile;
    let members = it.size, hidden = it.size;
    const relations: [number, number][] = [];
    for (let a = 0; a < t.nAggs; a++) {
      if (t.aggItem[a] !== it.index) continue;
      members = t.aggMembers[a];
      hidden = t.aggHidden[a];
      for (let k = t.aggRelOff[a]; k < t.aggRelOff[a + 1]; k++) relations.push([t.aggRel[k], t.aggRelCount[k]]);
    }
    relations.sort((a, b) => b[1] - a[1]);
    return { item: it, members, hidden, relations, central: centralOf(t, it.index) };
  }

  /** Aggregated edges between two items over the loaded tiles at the current zoom. */
  edgesBetween(a: MapItem, b: MapItem): Map<string, number> {
    const out = new Map<string, number>();
    for (const e of this.store.readyAt(this.curZ)) {
      const t = e.tile!;
      for (let k = 0; k < t.nEdges; k++) {
        const s = t.strings[t.id[t.edgeSrc[k]]], d = t.strings[t.id[t.edgeDst[k]]];
        if ((s === a.id && d === b.id) || (s === b.id && d === a.id)) {
          const key = `${t.edgeRel[k]}:${t.edgeState[k]}`;
          // Both endpoint tiles carry the edge: keep the max, not the sum.
          out.set(key, Math.max(out.get(key) ?? 0, t.edgeCount[k]));
        }
      }
    }
    return out;
  }

  findItem(id: string): MapItem | undefined {
    return this.byId.get(id) ?? this.items.find((it) => it.id === id);
  }

  get frameMs(): number {
    return this.lastFrameMs;
  }

  // ---- input -----------------------------------------------------------------

  private pick(sx: number, sy: number): MapItem | null {
    let best: MapItem | null = null;
    let bestD = Infinity;
    for (const it of this.items) {
      const [px, py] = this.camera.toScreen(it.x, it.y);
      const d = Math.hypot(px - sx, py - sy);
      const r = this.radiusOf(it) + 4;
      if (d <= r && d < bestD) {
        best = it;
        bestD = d;
      }
    }
    if (best) return best;
    // Labels are targets too.
    for (let i = 0; i < this.placed.length; i++) {
      const p = this.placed[i];
      const it = this.placedItems[i];
      if (it && sx >= p.x && sx <= p.x + p.w && sy >= p.y && sy <= p.y + p.h) return it;
    }
    return null;
  }

  private bindPointer(): void {
    const c = this.canvas;
    const local = (e: PointerEvent | WheelEvent): [number, number] => {
      const r = c.getBoundingClientRect();
      return [e.clientX - r.left, e.clientY - r.top];
    };
    c.addEventListener("wheel", (e) => {
      e.preventDefault();
      const [sx, sy] = local(e);
      const k = e.deltaMode === 1 ? 0.05 : 0.0018;
      this.camera.zoomAt(sx, sy, Math.exp(-e.deltaY * k));
      this.invalidate();
    }, { passive: false });
    c.addEventListener("pointerdown", (e) => {
      c.setPointerCapture(e.pointerId);
      this.pinch.set(e.pointerId, local(e));
      this.drag = { x: e.clientX, y: e.clientY, moved: false };
    });
    c.addEventListener("pointermove", (e) => {
      const p = local(e);
      if (this.pinch.size === 2 && this.pinch.has(e.pointerId)) {
        const [a, b] = [...this.pinch.values()];
        const before = Math.hypot(a[0] - b[0], a[1] - b[1]);
        this.pinch.set(e.pointerId, p);
        const [a2, b2] = [...this.pinch.values()];
        const after = Math.hypot(a2[0] - b2[0], a2[1] - b2[1]);
        if (before > 0) this.camera.zoomAt((a2[0] + b2[0]) / 2, (a2[1] + b2[1]) / 2, after / before);
        this.invalidate();
        return;
      }
      if (this.drag && c.hasPointerCapture(e.pointerId)) {
        const dx = e.clientX - this.drag.x, dy = e.clientY - this.drag.y;
        if (Math.abs(dx) + Math.abs(dy) > 2) this.drag.moved = true;
        this.drag.x = e.clientX;
        this.drag.y = e.clientY;
        if (this.drag.moved) {
          this.camera.pan(dx, dy);
          this.invalidate();
        }
        return;
      }
      const h = this.pick(p[0], p[1]);
      if (h !== this.hover) {
        this.hover = h;
        c.style.cursor = h ? "pointer" : "grab";
        // Explore shows no library names or ids, not even as a tooltip.
        c.title = h && this.isLettered(h) ? h.label : "";
        this.invalidate();
      }
    });
    const end = (e: PointerEvent): void => {
      this.pinch.delete(e.pointerId);
      const d = this.drag;
      this.drag = null;
      if (d && !d.moved && e.type === "pointerup") {
        const [sx, sy] = local(e);
        const it = this.pick(sx, sy);
        if (it) this.events.onSelect(it, e.shiftKey);
      }
    };
    c.addEventListener("pointerup", end);
    c.addEventListener("pointercancel", end);
    c.addEventListener("dblclick", (e) => {
      const [sx, sy] = local(e as unknown as PointerEvent);
      const [wx, wy] = this.camera.toWorld(sx, sy);
      this.camera.flyTo({ cx: wx, cy: wy, scale: this.camera.scale * 2.5 });
      this.invalidate();
    });
    c.addEventListener("keydown", (e) => this.onKey(e));
  }

  /** Keys move the camera at once: nothing animates on a (repeatable) key. */
  private onKey(e: KeyboardEvent): void {
    const c = this.camera;
    const handled = (() => {
      switch (e.key) {
        case "ArrowLeft": if (e.altKey) return false; c.pan(KEY_PAN_PX, 0); return true;
        case "ArrowRight": c.pan(-KEY_PAN_PX, 0); return true;
        case "ArrowUp": c.pan(0, KEY_PAN_PX); return true;
        case "ArrowDown": c.pan(0, -KEY_PAN_PX); return true;
        case "+": case "=": c.zoomAt(c.width / 2, c.height / 2, KEY_ZOOM); return true;
        case "-": case "_": c.zoomAt(c.width / 2, c.height / 2, 1 / KEY_ZOOM); return true;
        case "0": this.resetView(false); return true;
        case "n": case "N": case "p": case "P": {
          const n = this.placedItems.length;
          if (!n) return true;
          const forward = e.key === "n" || e.key === "N";
          this.kbIndex = forward ? (this.kbIndex + 1) % n : (this.kbIndex - 1 + n) % n;
          return true;
        }
        case "Enter": {
          const it = this.placedItems[this.kbIndex];
          if (it) this.events.onSelect(it, e.shiftKey);
          return true;
        }
        default: return false;
      }
    })();
    if (handled) {
      e.preventDefault();
      this.invalidate();
    }
  }

  // ---- benchmark: N synthetic points + N edges, camera animated -------------------

  async bench(n: number, seconds: number): Promise<FpsReport> {
    const pts = new Float32Array(n * 4);
    const edges = new Float32Array(n * 6);
    let s = 12345;
    const rnd = (): number => ((s = (s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
    const synth: MapItem[] = [];
    const fake = { strings: [] } as unknown as Tile;
    for (let i = 0; i < n; i++) {
      // Clustered positions, like a real layout: 64 gaussian blobs.
      const c = i % 64;
      const cx = 0.1 + 0.8 * ((c % 8) / 7), cy = 0.1 + 0.8 * (Math.floor(c / 8) / 7);
      const r = Math.sqrt(-2 * Math.log(rnd() + 1e-9)) * 0.03, a = rnd() * Math.PI * 2;
      pts.set([cx + r * Math.cos(a), cy + r * Math.sin(a), 3, i % 18], i * 4);
      synth.push({ id: `b:${i}`, label: `node ${i}`, kind: "entity", type: i % 18, x: pts[i * 4], y: pts[i * 4 + 1], size: 1, priority: i, community: "", tile: fake, index: i });
    }
    for (let i = 0; i < n; i++) {
      const j = (i * 7919 + 13) % n;
      edges.set([pts[i * 4], pts[i * 4 + 1], pts[j * 4], pts[j * 4 + 1], i % 5, 1], i * 6);
    }
    this.items = synth;
    this.itemsGen = -2;
    this.benchLayer = this.renderer.createLayer(pts, edges);
    const frames: number[] = [];
    const cpu: number[] = [];
    const labels: number[] = [];
    const t0 = performance.now();
    await new Promise<void>((done) => {
      let last = t0;
      const tick = (now: number): void => {
        const u = (now - t0) / 1000;
        // Continuous pan and zoom between the full view and 16x.
        this.camera.set({ cx: 0.5 + 0.25 * Math.sin(u * 0.7), cy: 0.5 + 0.25 * Math.cos(u * 0.5), scale: this.camera.fitScale() * 2 ** (2 + 2 * Math.sin(u * 0.9)) });
        const c0 = performance.now();
        this.drawBench();
        cpu.push(performance.now() - c0);
        labels.push(this.placed.length);
        frames.push(now - last);
        last = now;
        if (u < seconds) requestAnimationFrame(tick);
        else done();
      };
      requestAnimationFrame(tick);
    });
    const secs = (performance.now() - t0) / 1000;
    this.renderer.deleteLayer(this.benchLayer);
    this.benchLayer = null;
    frames.shift();
    const q = (xs: number[], p: number): number => {
      const v = [...xs].sort((a, b) => a - b);
      return v[Math.min(v.length - 1, Math.floor(p * (v.length - 1)))];
    };
    return {
      frames: frames.length, seconds: secs, fps: frames.length / secs,
      p50_ms: q(frames, 0.5), p95_ms: q(frames, 0.95), p99_ms: q(frames, 0.99),
      cpu_p50_ms: q(cpu, 0.5), cpu_p95_ms: q(cpu, 0.95), points: n, edges: n, labels_p50: q(labels, 0.5),
      renderer: this.rendererName, width: this.camera.width, height: this.camera.height, dpr: devicePixelRatio || 1,
    };
  }

  private drawBench(): void {
    const f = this.frameInfo();
    this.renderer.begin(f);
    this.renderer.drawEdges([[this.benchLayer!, 1]], f, 0.25);
    this.renderer.drawPoints([[this.benchLayer!, 1]], f);
    // Labels over the synthetic items, same placement code as the real map.
    const ctx = this.ctx;
    const dpr = devicePixelRatio || 1;
    const { width, height } = this.camera;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const cands: LabelCandidate[] = [];
    const letters: Lettering[] = [];
    const vis: MapItem[] = [];
    for (const it of this.items) {
      const [sx, sy] = this.camera.toScreen(it.x, it.y);
      if (sx < 0 || sy < 0 || sx > width || sy > height) continue;
      cands.push({ key: vis.length, x: sx, y: sy, r: 3, w: this.textWidth(it.label, BENCH_STYLE), h: labelHeight(BENCH_STYLE) });
      letters.push({ text: it.label, style: BENCH_STYLE, color: this.pal["text-primary"] });
      vis.push(it);
      if (cands.length >= MAX_LABEL_CANDIDATES) break;
    }
    this.placed = placeLabels(cands, width, height, BENCH_MAX_LABELS);
    this.placedItems = this.placed.map((p) => vis[p.key]);
    this.paintLabels(this.placed, letters);
  }
}

export type { Entry };
