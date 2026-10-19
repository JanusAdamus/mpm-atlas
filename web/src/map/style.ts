// Paint decisions of the map that do not need a GPU or a DOM, so they can be tested:
// the edge palette per mode, edge opacity, and how neighbors of a focus are ranked.
// One color, one meaning (tokens.ts): gold says "validated" and nothing else.
import { RELATION_TYPES } from "../vocab.ts";
import { EVIDENCE, HISTORICAL, rgb, VEIL, type EvidenceMark, type Palette, type ThemeName } from "../tokens.ts";
import type { MapMode } from "./labels.ts";

/** Background edges in explore: one faint neutral, whatever their evidence state. */
export const NEUTRAL_EDGE: keyof Palette = "border-strong";
export const EDGE_STATES = 8;

/** Per-state edge colors for the GPU (8 x rgb).  Explore: all neutral, so the 244,933
 *  validated library links do not paint the background gold; technical: the theme's own. */
export function edgeStateColors(theme: ThemeName, mode: MapMode, themeColors: Float32Array): Float32Array {
  if (mode === "technical") return themeColors;
  const out = new Float32Array(EDGE_STATES * 3);
  const c = rgb(theme, NEUTRAL_EDGE);
  for (let i = 0; i < EDGE_STATES; i++) out.set(c, i * 3);
  return out;
}

/** Edge opacity: falls with the number of edges in view (dense views read as a tint, not a
 *  hairball); technical keeps state colors but lower, so validated links do not form a haze. */
const EDGE_REF = 900;
const EDGE_MIN_FACTOR = 0.28;
export function edgeAlpha(mode: MapMode, dark: boolean, edgesInView: number): number {
  const density = Math.max(EDGE_MIN_FACTOR, Math.min(1, Math.sqrt(EDGE_REF / Math.max(1, edgesInView))));
  const base = mode === "explore" ? (dark ? 0.2 : 0.22) : dark ? 0.1 : 0.12;
  return base * density;
}

/** Neighbor ranking: mathematical relations first, then area and use, then history, then
 *  bibliography (citations last). */
const RANK: Record<string, number> = {
  depends_on_logically: 0, proves: 0, prerequisite_for: 0, generalizes: 0, special_case_of: 0, equivalent_to: 0, counterexample_to: 0, formalized_as: 0,
  part_of_area: 1, analogous_to: 1, applied_in: 1, models: 1,
  influenced: 2, named_after: 2,
  cites: 3, appears_in: 3, authored_by: 3, member_of_concept: 3,
};
export const relationRank = (code: number): number => RANK[RELATION_TYPES[code] ?? ""] ?? 3;

export interface NeighborCandidate {
  id: string;
  rel: number; // relation code
  count: number; // claims behind the aggregated edge
}

/** At most `max` distinct neighbors: best relation rank first, then the most claims. */
export function rankNeighbors(cands: readonly NeighborCandidate[], max = 7): string[] {
  const best = new Map<string, { rank: number; count: number }>();
  for (const c of cands) {
    const r = relationRank(c.rel);
    const b = best.get(c.id);
    if (!b || r < b.rank || (r === b.rank && c.count > b.count)) best.set(c.id, { rank: r, count: c.count });
  }
  return [...best.entries()]
    .sort((a, b) => a[1].rank - b[1].rank || b[1].count - a[1].count || (a[0] < b[0] ? -1 : 1))
    .slice(0, max)
    .map(([id]) => id);
}

/** Stroke of one route hop (C4-01/C4-02): its own evidence state, so the map says what the
 *  step card says.  The long ink dash of «Histórica» only on a historical route; a hop
 *  whose state is unknown is drawn as a candidate (dotted), never as a solid line. */
export function routeHopMark(state: string | undefined, historical: boolean): EvidenceMark {
  if (historical) return HISTORICAL;
  return (state && EVIDENCE[state]) || EVIDENCE.model_proposed;
}

/** Point colors for the GPU (32 x rgb, index 31 = regions): one neutral ink for every point,
 *  a lighter one under the veil.  A point's color carries no meaning; its shape does. */
export function pointPalette(theme: ThemeName, veiled: boolean): Float32Array {
  const out = new Float32Array(32 * 3);
  const c = rgb(theme, veiled ? VEIL.point : VEIL.pointLit);
  for (let i = 0; i < 32; i++) out.set(c, i * 3);
  return out;
}
