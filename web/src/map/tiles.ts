// Tile streaming by viewport: fetch what the camera covers (nearest first, bounded
// concurrency), abort what scrolled away, keep an LRU of decoded tiles with their
// GPU buffers.  Turning a tile into GPU instances happens once, on arrival.
import { api } from "../api.ts";
import { KIND_CLUSTER, KIND_CONCEPT, NONE, type Tile } from "../tile.ts";
import { EDGE_FLOATS, POINT_FLOATS, type Layer, type Renderer } from "./gl.ts";

const MAX_TILES = 384;
const MAX_INFLIGHT = 8;

export interface TileParams {
  layer: string;
  lang: string;
  version: string;
}

export interface Entry {
  key: string;
  z: number;
  x: number;
  y: number;
  state: "loading" | "ready" | "error";
  tile?: Tile;
  gpu?: Layer;
  ctrl?: AbortController;
  readyAt: number;
  lastUsed: number;
  /** Item index -> dimmed by the current region focus. */
  dimmed?: Uint8Array;
}

export const tileKey = (z: number, x: number, y: number): string => `${z}/${x}/${y}`;

export function entityRadius(priority: number): number {
  return priority < 32 ? 4.5 : priority < 512 ? 3.5 : 2.75;
}

export function clusterRadius(hidden: number): number {
  return Math.min(34, Math.max(7, 4 + Math.sqrt(hidden) * 0.9));
}

/** A concept grows with its member declarations (stable across zooms). */
export function conceptRadius(members: number): number {
  return Math.min(18, 5.5 + Math.sqrt(members) * 0.35);
}

export function itemRadius(t: Tile, i: number): number {
  if (t.kind[i] === KIND_CLUSTER) return clusterRadius(t.size[i]);
  if (t.kind[i] === KIND_CONCEPT) return conceptRadius(t.size[i]);
  return entityRadius(t.priority[i] === NONE ? 1e9 : t.priority[i]);
}

/** Instance data of one tile.  Only the tile's own nodes are drawn as points;
 *  external endpoints exist to anchor edges that leave the tile. */
export function tileInstances(t: Tile, dimmed?: Uint8Array): { points: Float32Array; edges: Float32Array } {
  const points = new Float32Array(t.nNodes * POINT_FLOATS);
  for (let i = 0; i < t.nNodes; i++) {
    const o = i * POINT_FLOATS;
    const kind = t.kind[i];
    points[o] = t.px[i];
    points[o + 1] = t.py[i];
    points[o + 2] = itemRadius(t, i);
    points[o + 3] = (kind === KIND_CLUSTER ? 256 : t.type[i] & 31) + (dimmed?.[i] ? 512 : 0) + (kind === KIND_CONCEPT ? 1024 : 0);
  }
  // ponytail: an edge between two tiles is carried (and drawn) by both, so it is
  // slightly darker; dropping one copy loses the edge when the other tile is off-screen.
  const edges = new Float32Array(t.nEdges * EDGE_FLOATS);
  for (let e = 0; e < t.nEdges; e++) {
    const a = t.edgeSrc[e], b = t.edgeDst[e];
    const o = e * EDGE_FLOATS;
    edges[o] = t.px[a];
    edges[o + 1] = t.py[a];
    edges[o + 2] = t.px[b];
    edges[o + 3] = t.py[b];
    const dim = dimmed !== undefined && (a >= t.nNodes || dimmed[a]) && (b >= t.nNodes || dimmed[b]);
    edges[o + 4] = t.edgeState[e] + (dim ? 16 : 0);
    edges[o + 5] = 1 + Math.min(Math.log2(t.edgeCount[e]), 5) * 0.35;
  }
  return { points, edges };
}

export class TileStore {
  private readonly entries = new Map<string, Entry>();
  private inflight = 0;
  private params: TileParams | null = null;
  private focus: Set<string> | null = null;
  private readonly renderer: Renderer;
  private readonly onChange: () => void;
  /** Bumped whenever the set of ready tiles changes. */
  generation = 0;
  lastError: string | null = null;

  constructor(renderer: Renderer, onChange: () => void) {
    this.renderer = renderer;
    this.onChange = onChange;
  }

  setParams(p: TileParams): void {
    const old = this.params;
    if (old && old.layer === p.layer && old.lang === p.lang && old.version === p.version) return;
    this.params = p;
    this.clear();
  }

  clear(): void {
    for (const e of this.entries.values()) this.drop(e);
    this.entries.clear();
    this.generation++;
  }

  get(z: number, x: number, y: number): Entry | undefined {
    return this.entries.get(tileKey(z, x, y));
  }

  /** Request the tiles at zoom z (nearest first); abort loads nobody wants any more. */
  want(z: number, xy: [number, number][], keep: Set<string>): void {
    if (!this.params) return;
    const now = performance.now();
    for (const [x, y] of xy) {
      const k = tileKey(z, x, y);
      keep.add(k);
      const e = this.entries.get(k);
      if (e) {
        e.lastUsed = now;
        continue;
      }
      if (this.inflight >= MAX_INFLIGHT) continue;
      this.load(z, x, y, now);
    }
    for (const e of this.entries.values()) {
      if (e.state === "loading" && !keep.has(e.key)) {
        e.ctrl?.abort();
        this.entries.delete(e.key);
      }
    }
    this.evict(keep);
  }

  private load(z: number, x: number, y: number, now: number): void {
    const p = this.params!;
    const key = tileKey(z, x, y);
    const ctrl = new AbortController();
    const e: Entry = { key, z, x, y, state: "loading", ctrl, readyAt: 0, lastUsed: now };
    this.entries.set(key, e);
    this.inflight++;
    api
      .tile(p.layer, z, x, y, p.lang, p.version, ctrl.signal)
      .then((tile) => {
        if (this.entries.get(key) !== e) return;
        e.tile = tile;
        e.dimmed = this.dimmedFor(tile);
        const inst = tileInstances(tile, e.dimmed);
        e.gpu = this.renderer.createLayer(inst.points, inst.edges);
        e.state = "ready";
        e.readyAt = performance.now();
        this.generation++;
      })
      .catch((err: unknown) => {
        if (ctrl.signal.aborted) return;
        e.state = "error";
        this.lastError = err instanceof Error ? err.message : String(err);
      })
      .finally(() => {
        this.inflight--;
        e.ctrl = undefined;
        this.onChange();
      });
  }

  private evict(keep: Set<string>): void {
    if (this.entries.size <= MAX_TILES) return;
    const old = [...this.entries.values()].filter((e) => !keep.has(e.key) && e.state !== "loading");
    old.sort((a, b) => a.lastUsed - b.lastUsed);
    for (const e of old.slice(0, this.entries.size - MAX_TILES)) {
      this.drop(e);
      this.entries.delete(e.key);
    }
    this.generation++;
  }

  private drop(e: Entry): void {
    e.ctrl?.abort();
    if (e.gpu) this.renderer.deleteLayer(e.gpu);
    e.gpu = undefined;
  }

  /** Dim everything outside the focused communities (region focus). */
  setFocus(communities: Set<string> | null): void {
    this.focus = communities;
    for (const e of this.entries.values()) {
      if (e.state !== "ready" || !e.tile || !e.gpu) continue;
      e.dimmed = this.dimmedFor(e.tile);
      const inst = tileInstances(e.tile, e.dimmed);
      this.renderer.updatePoints(e.gpu, inst.points);
      this.renderer.updateEdges(e.gpu, inst.edges);
    }
    this.generation++;
  }

  private dimmedFor(t: Tile): Uint8Array | undefined {
    const f = this.focus;
    if (!f) return undefined;
    const d = new Uint8Array(t.nItems);
    for (let i = 0; i < t.nItems; i++) d[i] = f.has(t.strings[t.community[i]]) ? 0 : 1;
    return d;
  }

  readyAt(z: number): Entry[] {
    return [...this.entries.values()].filter((e) => e.z === z && e.state === "ready");
  }
}
