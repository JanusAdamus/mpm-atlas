// 2-D camera over the layout's unit square.  `scale` is CSS px per world unit; the
// tile zoom follows it so a tile is drawn at about TILE_PX on screen.

export const TILE_PX = 384;
const MIN_SCALE_FACTOR = 0.5; // of the fit-to-screen scale
const MAX_SCALE = 1 << 24;

export interface View {
  cx: number;
  cy: number;
  scale: number;
}

export class Camera {
  cx = 0.5;
  cy = 0.5;
  scale = 512;
  width = 1;
  height = 1;
  private anim: { from: View; to: View; t0: number; ms: number } | null = null;

  resize(width: number, height: number): void {
    const first = this.width === 1;
    this.width = Math.max(1, width);
    this.height = Math.max(1, height);
    if (first) this.scale = this.fitScale();
  }

  fitScale(): number {
    return Math.min(this.width, this.height) * 0.92;
  }

  view(): View {
    return { cx: this.cx, cy: this.cy, scale: this.scale };
  }

  set(v: View): void {
    this.cx = clamp(v.cx, -0.5, 1.5);
    this.cy = clamp(v.cy, -0.5, 1.5);
    this.scale = clamp(v.scale, this.fitScale() * MIN_SCALE_FACTOR, MAX_SCALE);
  }

  toScreen(x: number, y: number): [number, number] {
    return [(x - this.cx) * this.scale + this.width / 2, (y - this.cy) * this.scale + this.height / 2];
  }

  toWorld(sx: number, sy: number): [number, number] {
    return [(sx - this.width / 2) / this.scale + this.cx, (sy - this.height / 2) / this.scale + this.cy];
  }

  /** World rectangle on screen: [x0, y0, x1, y1]. */
  bounds(): [number, number, number, number] {
    const [x0, y0] = this.toWorld(0, 0);
    const [x1, y1] = this.toWorld(this.width, this.height);
    return [x0, y0, x1, y1];
  }

  /** Continuous tile zoom; the integer part is the tile level to fetch. */
  zoomFloat(): number {
    return Math.log2(this.scale / TILE_PX);
  }

  tileZoom(maxZoom: number): number {
    return clamp(Math.round(this.zoomFloat()), 0, maxZoom);
  }

  pan(dxPx: number, dyPx: number): void {
    this.anim = null;
    this.set({ cx: this.cx - dxPx / this.scale, cy: this.cy - dyPx / this.scale, scale: this.scale });
  }

  /** Zoom by `factor` keeping the world point under (sx, sy) fixed. */
  zoomAt(sx: number, sy: number, factor: number): void {
    this.anim = null;
    const [wx, wy] = this.toWorld(sx, sy);
    const scale = clamp(this.scale * factor, this.fitScale() * MIN_SCALE_FACTOR, MAX_SCALE);
    this.set({ cx: wx - (sx - this.width / 2) / scale, cy: wy - (sy - this.height / 2) / scale, scale });
  }

  flyTo(to: View, ms = 600): void {
    this.anim = { from: this.view(), to, t0: performance.now(), ms };
  }

  /** Fit the world rectangle with a margin. */
  fit(x0: number, y0: number, x1: number, y1: number, ms = 600): void {
    const w = Math.max(x1 - x0, 1e-6), h = Math.max(y1 - y0, 1e-6);
    const scale = Math.min(this.width / w, this.height / h) * 0.7;
    // Shift the content left a little: route stop names are drawn to the right of their markers.
    this.flyTo({ cx: (x0 + x1) / 2 + 0.08 * w, cy: (y0 + y1) / 2, scale }, ms);
  }

  /** Advance an animation; true while one is running. */
  step(now: number): boolean {
    if (!this.anim) return false;
    const { from, to, t0, ms } = this.anim;
    const u = clamp((now - t0) / ms, 0, 1);
    const e = u < 0.5 ? 4 * u * u * u : 1 - (-2 * u + 2) ** 3 / 2;
    // Interpolate zoom in log space so the motion feels uniform across scales.
    const ls = Math.log(from.scale) + (Math.log(to.scale) - Math.log(from.scale)) * e;
    this.set({ cx: from.cx + (to.cx - from.cx) * e, cy: from.cy + (to.cy - from.cy) * e, scale: Math.exp(ls) });
    if (u >= 1) this.anim = null;
    return true;
  }

  get animating(): boolean {
    return this.anim !== null;
  }
}

export function clamp(v: number, lo: number, hi: number): number {
  return v < lo ? lo : v > hi ? hi : v;
}

/** Tiles (x, y) at zoom z covering the world rectangle, nearest to its center first. */
export function coveringTiles(z: number, b: [number, number, number, number], margin = 0): [number, number][] {
  const n = 1 << z;
  const x0 = clamp(Math.floor(b[0] * n) - margin, 0, n - 1), x1 = clamp(Math.floor(b[2] * n) + margin, 0, n - 1);
  const y0 = clamp(Math.floor(b[1] * n) - margin, 0, n - 1), y1 = clamp(Math.floor(b[3] * n) + margin, 0, n - 1);
  if (b[2] < 0 || b[3] < 0 || b[0] > 1 || b[1] > 1) return [];
  const cx = ((b[0] + b[2]) / 2) * n, cy = ((b[1] + b[3]) / 2) * n;
  const out: [number, number][] = [];
  for (let x = x0; x <= x1; x++) for (let y = y0; y <= y1; y++) out.push([x, y]);
  out.sort((a, b2) => Math.hypot(a[0] + 0.5 - cx, a[1] + 0.5 - cy) - Math.hypot(b2[0] + 0.5 - cx, b2[1] + 0.5 - cy));
  return out;
}
