// Level-of-detail labels with collision avoidance: candidates arrive in priority
// order (regions by size, then entities by the layout's label rank); each is
// placed at the first free spot of a few anchors around its point, or skipped.
// Occupancy is a uniform grid of buckets, so a placement costs O(labels nearby).

export interface LabelCandidate {
  key: number; // caller's handle (index into its item list)
  x: number; // screen px of the point
  y: number;
  r: number; // point radius px
  w: number; // text width px
  h: number; // text height px
}

export interface PlacedLabel {
  key: number;
  x: number; // text box top-left
  y: number;
  w: number;
  h: number;
}

const CELL = 64;
const PAD = 2;
/** Right, left, above, below the point. */
const ANCHORS: ((c: LabelCandidate) => [number, number])[] = [
  (c) => [c.x + c.r + 3, c.y - c.h / 2],
  (c) => [c.x - c.r - 3 - c.w, c.y - c.h / 2],
  (c) => [c.x - c.w / 2, c.y - c.r - 2 - c.h],
  (c) => [c.x - c.w / 2, c.y + c.r + 2],
];

export function placeLabels(cands: Iterable<LabelCandidate>, width: number, height: number, max: number): PlacedLabel[] {
  const cols = Math.ceil(width / CELL) + 1;
  const rows = Math.ceil(height / CELL) + 1;
  const grid: PlacedLabel[][] = Array.from({ length: cols * rows }, () => []);
  const placed: PlacedLabel[] = [];
  const cellsOf = (b: PlacedLabel, fn: (cell: PlacedLabel[]) => boolean): boolean => {
    const c0 = Math.max(0, Math.floor(b.x / CELL)), c1 = Math.min(cols - 1, Math.floor((b.x + b.w) / CELL));
    const r0 = Math.max(0, Math.floor(b.y / CELL)), r1 = Math.min(rows - 1, Math.floor((b.y + b.h) / CELL));
    for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) if (fn(grid[r * cols + c])) return true;
    return false;
  };
  const overlaps = (a: PlacedLabel, b: PlacedLabel): boolean =>
    a.x < b.x + b.w + PAD && b.x < a.x + a.w + PAD && a.y < b.y + b.h + PAD && b.y < a.y + a.h + PAD;
  for (const c of cands) {
    if (placed.length >= max) break;
    for (const anchor of ANCHORS) {
      const [x, y] = anchor(c);
      const box = { key: c.key, x, y, w: c.w, h: c.h };
      if (x < 0 || y < 0 || x + c.w > width || y + c.h > height) continue;
      if (cellsOf(box, (cell) => cell.some((o) => overlaps(o, box)))) continue;
      cellsOf(box, (cell) => {
        cell.push(box);
        return false;
      });
      placed.push(box);
      break;
    }
  }
  return placed;
}
