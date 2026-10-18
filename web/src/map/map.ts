// The multiscale map: camera + tile streaming + WebGL2 layers + a 2-D overlay for
// labels, focus rings and the path.  Zoom is continuous; when the tile level
// changes, the new level fades in over the old one, which fades out once the
// viewport is covered, so regions dissolve into their members instead of popping.
import { centralOf, KIND_CLUSTER, NONE, type Tile } from "../tile.ts";
import { Camera, coveringTiles, type View } from "./camera.ts";
import { Renderer, type Frame, type Layer, type Theme } from "./gl.ts";
import { placeLabels, type LabelCandidate, type PlacedLabel } from "./labels.ts";
import { clusterRadius, entityRadius, TileStore, type Entry } from "./tiles.ts";

const FADE_IN_MS = 220;
const FADE_OUT_MS = 260;
const MAX_LABELS = 220;
const MAX_LABEL_CANDIDATES = 5000;
const LABEL_FONT = "12px system-ui, -apple-system, 'Segoe UI', sans-serif";
const REGION_FONT = "600 13px system-ui, -apple-system, 'Segoe UI', sans-serif";
const KEY_PAN_PX = 80;
const KEY_ZOOM = 1.5;
const LABEL_CHARS = 34;

export interface MapItem {
  id: string;
  label: string;
  kind: "entity" | "cluster";
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
    kind: t.kind[i] === KIND_CLUSTER ? "cluster" : "entity",
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

const short = (s: string): string => (s.length > LABEL_CHARS ? `${s.slice(0, LABEL_CHARS - 1)}…` : s);

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
  private itemsGen = -1;
  private itemsZ = -1;
  private placed: PlacedLabel[] = [];
  private placedItems: MapItem[] = [];
  private widths = new Map<string, number>();
  private hover: MapItem | null = null;
  private kbIndex = -1;
  private selected: { x: number; y: number } | null = null;
  private path: PathMark[] | null = null;
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
    this.renderer.setTheme(theme);
    this.invalidate();
  }

  setFocus(communities: string[] | null): void {
    this.store.setFocus(communities ? new Set(communities) : null);
    this.invalidate();
  }

  setSelected(pos: { x: number; y: number } | null): void {
    this.selected = pos;
    this.invalidate();
  }

  setPath(marks: PathMark[] | null): void {
    this.path = marks;
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

  flyTo(x: number, y: number, scale?: number): void {
    this.camera.flyTo({ cx: x, cy: y, scale: scale ?? Math.max(this.camera.scale, 384 * 2 ** Math.min(this.maxZoom, 6)) });
    this.invalidate();
  }

  setView(v: View, animate = false): void {
    if (animate) this.camera.flyTo(v);
    else this.camera.set(v);
    this.invalidate();
  }

  resetView(): void {
    this.camera.flyTo({ cx: 0.5, cy: 0.5, scale: this.camera.fitScale() });
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
    const f = this.frameInfo();
    this.renderer.begin(f);
    const edgeAlpha = this.dark ? 0.55 : 0.45;
    this.renderer.drawEdges([...prev, ...cur], f, edgeAlpha);
    this.renderer.drawPoints([...prev, ...cur], f);
    this.drawOverlay(z);
    const loading = need.some(([x, y]) => this.store.get(z, x, y)?.state === "loading");
    return fading || loading;
  }

  /** Items of the ready tiles at the current zoom, in label priority order. */
  private refreshItems(z: number): void {
    if (this.itemsGen === this.store.generation && this.itemsZ === z) return;
    this.itemsGen = this.store.generation;
    this.itemsZ = z;
    const out: MapItem[] = [];
    for (const e of this.store.readyAt(z)) {
      const t = e.tile!;
      for (let i = 0; i < t.nNodes; i++) out.push(itemOf(t, i));
    }
    out.sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "cluster" ? -1 : 1;
      return a.kind === "cluster" ? b.size - a.size : a.priority - b.priority;
    });
    this.items = out;
    this.kbIndex = -1;
  }

  private radiusOf(it: MapItem): number {
    return it.kind === "cluster" ? clusterRadius(it.size) : entityRadius(it.priority === NONE ? 1e9 : it.priority);
  }

  private textWidth(s: string, cluster: boolean): number {
    const key = (cluster ? "R" : "E") + s;
    let w = this.widths.get(key);
    if (w === undefined) {
      if (this.widths.size > 50_000) this.widths.clear();
      this.ctx.font = cluster ? REGION_FONT : LABEL_FONT;
      w = this.ctx.measureText(s).width;
      this.widths.set(key, w);
    }
    return w;
  }

  private drawOverlay(z: number): void {
    this.refreshItems(z);
    const ctx = this.ctx;
    const dpr = devicePixelRatio || 1;
    const { width, height } = this.camera;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const cands: LabelCandidate[] = [];
    const visible: MapItem[] = [];
    for (const it of this.items) {
      const [sx, sy] = this.camera.toScreen(it.x, it.y);
      if (sx < -40 || sy < -20 || sx > width + 40 || sy > height + 20) continue;
      const s = short(it.label);
      cands.push({ key: visible.length, x: sx, y: sy, r: this.radiusOf(it), w: this.textWidth(s, it.kind === "cluster"), h: 14 });
      visible.push(it);
      if (cands.length >= MAX_LABEL_CANDIDATES) break;
    }
    this.placed = placeLabels(cands, width, height, MAX_LABELS);
    this.placedItems = this.placed.map((p) => visible[p.key]);
    this.paintLabels(this.placed, this.placedItems);
    this.paintMarks();
  }

  private css(name: string): string {
    return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  }

  private paintLabels(placed: PlacedLabel[], items: MapItem[]): void {
    const ctx = this.ctx;
    const fg = this.css("--fg") || "#111";
    const halo = this.css("--halo") || "#fff";
    ctx.textBaseline = "top";
    ctx.lineJoin = "round";
    ctx.lineWidth = 3;
    for (let i = 0; i < placed.length; i++) {
      const p = placed[i], it = items[i];
      ctx.font = it.kind === "cluster" ? REGION_FONT : LABEL_FONT;
      const s = short(it.label);
      ctx.strokeStyle = halo;
      ctx.strokeText(s, p.x, p.y + 1);
      ctx.fillStyle = fg;
      ctx.fillText(s, p.x, p.y + 1);
    }
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

  private paintMarks(): void {
    const ctx = this.ctx;
    const accent = this.css("--accent") || "#0a58ca";
    const fg = this.css("--fg") || "#111";
    if (this.path && this.path.length) {
      ctx.beginPath();
      this.path.forEach((m, i) => {
        const [sx, sy] = this.camera.toScreen(m.x, m.y);
        if (i === 0) ctx.moveTo(sx, sy);
        else ctx.lineTo(sx, sy);
      });
      ctx.strokeStyle = accent;
      ctx.lineWidth = 3;
      ctx.stroke();
      this.path.forEach((m, i) => {
        const [sx, sy] = this.camera.toScreen(m.x, m.y);
        ctx.beginPath();
        ctx.arc(sx, sy, 9, 0, Math.PI * 2);
        ctx.fillStyle = accent;
        ctx.fill();
        ctx.fillStyle = "#fff";
        ctx.font = "700 11px system-ui, sans-serif";
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(String(i + 1), sx, sy + 0.5);
        ctx.textAlign = "start";
      });
    }
    this.compare.forEach((it, i) => this.ring(it.x, it.y, this.radiusOf(it) + 6, accent, 3, i === 0 ? [] : [6, 4]));
    if (this.selected) this.ring(this.selected.x, this.selected.y, 11, accent, 3);
    if (this.hover) this.ring(this.hover.x, this.hover.y, this.radiusOf(this.hover) + 4, fg, 1.5);
    const kb = this.placedItems[this.kbIndex];
    if (kb) {
      this.ring(kb.x, kb.y, this.radiusOf(kb) + 7, accent, 3, [3, 3]);
      this.canvas.setAttribute("aria-label", `${kb.label} (${kb.kind})`);
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
    return this.items.find((it) => it.id === id);
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
      if (sx >= p.x && sx <= p.x + p.w && sy >= p.y && sy <= p.y + p.h) return this.placedItems[i];
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
        c.title = h ? h.label : "";
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
      this.camera.flyTo({ cx: wx, cy: wy, scale: this.camera.scale * 2.5 }, 350);
      this.invalidate();
    });
    c.addEventListener("keydown", (e) => this.onKey(e));
  }

  private onKey(e: KeyboardEvent): void {
    const c = this.camera;
    const handled = (() => {
      switch (e.key) {
        case "ArrowLeft": if (e.altKey) return false; c.pan(KEY_PAN_PX, 0); return true;
        case "ArrowRight": c.pan(-KEY_PAN_PX, 0); return true;
        case "ArrowUp": c.pan(0, KEY_PAN_PX); return true;
        case "ArrowDown": c.pan(0, -KEY_PAN_PX); return true;
        case "+": case "=": c.flyTo({ ...c.view(), scale: c.scale * KEY_ZOOM }, 200); return true;
        case "-": case "_": c.flyTo({ ...c.view(), scale: c.scale / KEY_ZOOM }, 200); return true;
        case "0": this.resetView(); return true;
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
    const vis: MapItem[] = [];
    for (const it of this.items) {
      const [sx, sy] = this.camera.toScreen(it.x, it.y);
      if (sx < 0 || sy < 0 || sx > width || sy > height) continue;
      cands.push({ key: vis.length, x: sx, y: sy, r: 3, w: this.textWidth(it.label, false), h: 14 });
      vis.push(it);
      if (cands.length >= MAX_LABEL_CANDIDATES) break;
    }
    this.placed = placeLabels(cands, width, height, MAX_LABELS);
    this.placedItems = this.placed.map((p) => vis[p.key]);
    this.paintLabels(this.placed, this.placedItems);
  }
}

export type { Entry };
