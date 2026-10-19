// Level-of-detail labels with collision avoidance: candidates arrive in priority
// order (pinned labels first, then areas by size, then concepts by the layout's label
// rank); each is placed at the first free spot of a few anchors around its point, or
// skipped.  Occupancy is a uniform grid of buckets, so a placement costs O(labels nearby).
//
// Round 4 "printed atlas" hierarchy (docs/design/r4/guia-skills-diseno.md §2.2): areas are
// continents in letter-spaced small caps, concepts are cities, declarations appear only in
// the technical mode and at deep zoom.  Each zoom has a label budget.
import { MAP_LABELS, VEIL, type MapLabelStyle } from "../tokens.ts";

export interface LabelCandidate {
  key: number; // caller's handle (index into its item list)
  x: number; // screen px of the point
  y: number;
  r: number; // point radius px
  w: number; // text width px
  h: number; // text height px
  /** "center": centered on the point first (area names), then the usual anchors. */
  anchor?: "center";
  /** Index into `groupMax` of placeLabels (e.g. 0 = areas): caps how many of a kind are placed. */
  group?: number;
}

export interface PlacedLabel {
  key: number;
  x: number; // text box top-left
  y: number;
  w: number;
  h: number;
}

const CELL = 64;
/** Space kept between two text boxes: covers the 3 px paper halo (1.5 px each side). */
export const LABEL_PAD = 2;
/** Right, left, above, below the point. */
const ANCHORS: ((c: LabelCandidate) => [number, number])[] = [
  (c) => [c.x + c.r + 3, c.y - c.h / 2],
  (c) => [c.x - c.r - 3 - c.w, c.y - c.h / 2],
  (c) => [c.x - c.w / 2, c.y - c.r - 2 - c.h],
  (c) => [c.x - c.w / 2, c.y + c.r + 2],
];
const CENTER = (c: LabelCandidate): [number, number] => [c.x - c.w / 2, c.y - c.h / 2];
const CENTERED = [CENTER, ...ANCHORS];

export function placeLabels(cands: Iterable<LabelCandidate>, width: number, height: number, max: number, groupMax: readonly number[] = []): PlacedLabel[] {
  const cols = Math.ceil(width / CELL) + 1;
  const rows = Math.ceil(height / CELL) + 1;
  const grid: PlacedLabel[][] = Array.from({ length: cols * rows }, () => []);
  const placed: PlacedLabel[] = [];
  const perGroup = groupMax.map(() => 0);
  // Cells of the box grown by the pad: a neighbor within LABEL_PAD may sit in the next cell.
  const cellsOf = (b: PlacedLabel, fn: (cell: PlacedLabel[]) => boolean): boolean => {
    const c0 = Math.max(0, Math.floor((b.x - LABEL_PAD) / CELL)), c1 = Math.min(cols - 1, Math.floor((b.x + b.w + LABEL_PAD) / CELL));
    const r0 = Math.max(0, Math.floor((b.y - LABEL_PAD) / CELL)), r1 = Math.min(rows - 1, Math.floor((b.y + b.h + LABEL_PAD) / CELL));
    for (let r = r0; r <= r1; r++) for (let c = c0; c <= c1; c++) if (fn(grid[r * cols + c])) return true;
    return false;
  };
  const overlaps = (a: PlacedLabel, b: PlacedLabel): boolean =>
    a.x < b.x + b.w + LABEL_PAD && b.x < a.x + a.w + LABEL_PAD && a.y < b.y + b.h + LABEL_PAD && b.y < a.y + a.h + LABEL_PAD;
  for (const c of cands) {
    if (placed.length >= max) break;
    const g = c.group;
    if (g !== undefined && g < groupMax.length && perGroup[g] >= groupMax[g]) continue;
    for (const anchor of c.anchor === "center" ? CENTERED : ANCHORS) {
      const [x, y] = anchor(c);
      const box = { key: c.key, x, y, w: c.w, h: c.h };
      if (x < 0 || y < 0 || x + c.w > width || y + c.h > height) continue;
      if (cellsOf(box, (cell) => cell.some((o) => overlaps(o, box)))) continue;
      cellsOf(box, (cell) => {
        cell.push(box);
        return false;
      });
      placed.push(box);
      if (g !== undefined && g < groupMax.length) perGroup[g]++;
      break;
    }
  }
  return placed;
}

/** Map-only label text: drops the source prefix of Wikipedia fragments (they are drawn
 *  in muted italic instead) and the `Mathlib/` root of library areas.  Panels, search
 *  and the text view keep the full label. */
export function displayLabel(label: string): string {
  return label.replace(/^Wikipedia \([a-z-]{2,12}\): /, "").replace(/^Mathlib\//, "");
}

/** A Wikipedia article title ("Wikipedia (en): Temperature"): a source, never a region name. */
export const isArticleLabel = (label: string): boolean => /^Wikipedia \([a-z-]{2,12}\): /.test(label);

/** Entity types drawn as points without a name on the map (R4 point 8): documents.  At the
 *  zooms past the general layer, Wikipedia fragments about units, chemistry or physics
 *  crowded out the mathematics; the general layer already leaves them out.  The point,
 *  its hover title, the click and the text index keep them: nothing is removed. */
export const UNLABELLED_TYPES: ReadonlySet<string> = new Set(["source_fragment", "publication"]);

export const hasMapLabel = (kind: "entity" | "cluster" | "concept", type: string): boolean =>
  kind !== "entity" || !UNLABELLED_TYPES.has(type);

// ---- round 4: modes, tiers, budgets ------------------------------------------------------

/** "explore" (default): no ids, no library names, no declarations.  "technical": everything. */
export type MapMode = "explore" | "technical";
export type LabelTier = "area" | "subarea" | "concept" | "statement";

/** Id prefixes of the formal library (declarations, modules, directories): their names are
 *  code, so explore never draws them and never draws an edge between two of them. */
const LIBRARY_PREFIXES = ["mathlib:", "mathlib-mod:", "mathlib-dir:", "mathlib-layer:", "lean-mod:"];
export const isLibraryId = (id: string): boolean => LIBRARY_PREFIXES.some((p) => id.startsWith(p));
/** A concept derived from Mathlib's module tree ("algebra › hom (Mathlib, derivado)"). */
export const isDerivedId = (id: string): boolean => id.startsWith("concept:");

/** Text that would read as jargon to a beginner even when the item itself is fine. */
const JARGON = /Mathlib|\((?:[^)]*, )?(?:derivad[oa]|derived)\)|\bQ\d+\b|^\w[\w-]*:\S|^[A-Z][\w'₀-₉]*(\.[\w'₀-₉]+)+$/;
export const isJargonLabel = (label: string): boolean => JARGON.test(label);

export interface TierInput {
  kind: "entity" | "cluster" | "concept";
  type: string; // entity type name, "" for clusters
  id: string;
  label: string;
}

/** Which rung of the atlas hierarchy an item is lettered at, or null for an unnamed point. */
export function labelTier(it: TierInput, mode: MapMode): LabelTier | null {
  const technical = mode === "technical";
  if (!hasMapLabel(it.kind, it.type)) return null;
  if (!technical && isJargonLabel(it.label)) return null;
  // Explore never letters a person or an article title as a place on the map (C4-13).
  if (!technical && (it.type === "person" || isArticleLabel(it.label))) return null;
  if (it.kind === "cluster") return "subarea";
  if (isLibraryId(it.id)) {
    if (!technical) return null;
    return it.type === "area" || it.type === "theory" ? "subarea" : "statement";
  }
  if (isDerivedId(it.id)) return technical ? "concept" : null;
  if (it.type === "area") return "area";
  return "concept";
}

/** Zoom from which declarations get a name (technical mode only): the server's detail zoom. */
export const STATEMENT_MIN_ZOOM = 4;
/** Screen area (CSS px²) per label: a phone gets fewer names than a desktop. */
const PX2_PER_LABEL = 14_000;
// z0-z2 are the general layer and the home view sits there (z2 at 1440x900): 30 names at most.
const TOTAL_BY_ZOOM = [30, 30, 30, 60, 100];
const AREAS_BY_ZOOM = [8, 10, 12, 10, 8];

/** Labels allowed at tile zoom z: `total` (all kinds) and `areas` (continents). */
export function labelBudget(z: number, width: number, height: number): { total: number; areas: number } {
  const i = Math.max(0, Math.min(TOTAL_BY_ZOOM.length - 1, z));
  const byArea = Math.max(8, Math.round((width * height) / PX2_PER_LABEL));
  return { total: Math.min(TOTAL_BY_ZOOM[i], byArea), areas: AREAS_BY_ZOOM[i] };
}

/** Lettering of a tier at zoom z (areas recede once the reader is past the overview). */
export function tierStyle(tier: LabelTier, z: number): MapLabelStyle {
  if (tier === "area") return z >= 3 ? MAP_LABELS.areaDeep : MAP_LABELS.area;
  return MAP_LABELS[tier];
}

/** Canvas font shorthand of a style (small caps and tracking are set separately). */
export function cssFont(s: MapLabelStyle, family: string): string {
  return `${s.italic ? "italic " : ""}${s.weight} ${s.size}px ${family}`;
}

/** Height of a label box: one line at the style's size. */
export const labelHeight = (s: MapLabelStyle): number => Math.ceil(s.size * 1.25);

/** Order of the tiers when labels compete for room. */
export const TIER_RANK: Record<LabelTier, number> = { area: 0, subarea: 1, concept: 2, statement: 3 };

/** Lettering of a label under the focus veil (C4-08): a text ink (>= 4.5:1), never bolder
 *  than 400 and never larger than 13 px, so it stays a step below the lit focus (17/600)
 *  and neighbors (14/500) by size and weight, not by color alone.  Cached per style. */
const VEILED = new Map<MapLabelStyle, MapLabelStyle>();
export function veiledStyle(s: MapLabelStyle): MapLabelStyle {
  let v = VEILED.get(s);
  if (!v) VEILED.set(s, (v = { ...s, weight: Math.min(s.weight, 400), size: Math.min(s.size, 13), color: VEIL.label }));
  return v;
}
