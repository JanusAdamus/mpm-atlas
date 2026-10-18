// Typed client of the JSON API (docs/SPEC.md §15, core/atlas-server).
import { decodeTile, type Tile } from "./tile.ts";

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  constructor(status: number, code: string, message: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

export interface SearchHit {
  id: string;
  type: string;
  label: string;
  matched: { field: string; lang: string; text: string };
  score: number;
  lanes: Record<string, { rank: number; score: number } | null>;
}
export interface SearchResponse { query: string; lang: string; limit: number; truncated: boolean; hits: SearchHit[] }

export interface Provenance {
  source_id: string; locator: string; extractor: string; extractor_version: string;
  license: string; retrieved_at: string; quote: string | null;
}
export interface EntityJson {
  id: string; type: string; labels: Record<string, string>; aliases: [string, string][];
  descriptions: Record<string, string>; statements: Record<string, string>; formula: string | null;
  external_ids: [string, string][]; provenance: Provenance[];
}
export interface ClaimJson {
  id: string; relation: string; participants: { role: string; entity: string }[];
  provenance: Provenance[]; evidence_state: string; assumptions: string[];
  validity_domain: string | null; alternative_group: string | null;
  supersedes: string | null; superseded_by: string | null;
  formal: { library: string; version: string; declaration: string; axioms: string[] } | null;
  confidence: { value: number; estimates: string; calibration: string } | null;
  historical_date: string | null; explanation: Record<string, string>; disagreements: string[];
  labels: Record<string, string>;
}
export interface SourceJson { id: string; title: string; license: string; uri?: string | null; version?: string | null; terms?: string | null }
export interface NodeRef { id: string; label: string; type: string; derived: boolean }
export interface EntityConceptInfo {
  /** Present when the entity is a concept of the snapshot (it groups declarations). */
  as_concept: { members: number; memberships: number; derived: boolean } | null;
  /** The concept a declaration is primarily assigned to. */
  primary_concept: NodeRef | null;
}
export interface EntityResponse {
  entity: EntityJson; canonical_id: string; merged_from: string[]; coverage: "known" | "unknown";
  label: string; counts_by_relation: Record<string, number>; limit: number; truncated: boolean;
  claims: ClaimJson[]; sources: Record<string, SourceJson>;
  /** Absent on servers without the concept layer. */
  concept?: EntityConceptInfo;
}

export interface HeuristicScore { value: number; estimates: string; calibration: string }
export interface ConceptMember {
  id: string; label: string; type: string; primary: boolean; claim_id: string; evidence_state: string;
  /** How the projection assigned it: curated, name_doc, resolve, namespace, signature, zone, zone_derived. */
  signal?: string | null; evidence?: unknown; rank?: number | null;
  /** Heuristic, not calibrated: never shown as a probability. */
  score?: HeuristicScore | null;
  provenance: { source_id: string; locator: string; license: string; extractor?: string; extractor_version?: string }[];
}
export interface ConceptEdgeStat { relation: string; evidence_state: string; count: number; example_claim: string }
export interface ConceptEdgeJson { concept: NodeRef; total: number; stats: ConceptEdgeStat[] }
export interface ConceptResponse {
  concept: NodeRef; description: string | null; members_total: number; memberships_total: number;
  memberships_by_state: Record<string, number>; offset: number; limit: number; truncated: boolean;
  members: ConceptMember[];
  edges_out_total: number; edges_out: ConceptEdgeJson[]; edges_in_total: number; edges_in: ConceptEdgeJson[];
}
export interface ConceptEdgesResponse {
  from: NodeRef; to: NodeRef; relation: string | null; total: number; complete: boolean; offset: number; limit: number;
  edges: { claim_id: string; relation: string; evidence_state: string; source: NodeRef; target: NodeRef }[];
}

export interface NeighborhoodNode { id: string; type: string; label: string; coverage: string; degree: number }
export interface NeighborhoodEdge {
  claim_id: string; relation: string; source: string; target: string; evidence_state: string;
  conjunct_count: number; alternative_group: string | null;
}
export interface NeighborhoodResponse {
  center: string; depth: number; limit: number; truncated: boolean;
  nodes: NeighborhoodNode[]; edges: NeighborhoodEdge[]; hidden: Record<string, number>;
}

export interface PathStep {
  claim_id: string; relation: string; from: string; to: string; from_label: string; to_label: string;
  direction: "forward" | "inverse"; evidence_state: string; authorizes: string; route_so_far: string;
  assumptions: string[]; premises: string[]; premises_missing: string[];
  premises_satisfied: { premise: string; how: string; claims: string[]; truncated: boolean }[];
  via: string[]; context: string[]; validity_domain: string | null; alternatives: string[];
  sources: { source_id: string; locator: string; license: string }[];
}
export interface Conclusion {
  authorizes: string; assumptions: string[]; weakest_evidence: string; is_hypothesis: boolean;
  kind: string; strength: number; mathematical: boolean; complete: boolean;
  /** Reader-facing class (mathematical, structural, historical, …); absent on older servers. */
  category?: string;
  missing_premises: string[]; caveats: string[];
}
export interface FoundPath { score: number; cost_terms: Record<string, number>; conclusion: Conclusion; steps: PathStep[] }
export interface PathResponse {
  from: string; to: string; objective: string; weights: Record<string, number>;
  abstained: boolean; reason: string | null; abstention: string | null;
  paths: FoundPath[]; exploration: FoundPath | null; composition_version: number;
  stats: { algorithm: string; expanded: number; micros: number; truncated: boolean };
}

export interface Health {
  status: string; version: string;
  snapshot: { content_hash: string; nodes: number; claims: number; edges: number };
  layout_version: string | null; components: Record<string, string>;
  /** Zoom semantics of the map (absent on older servers; null zooms without a concept layer). */
  map?: { max_zoom: number; general_zoom: number | null; detail_zoom: number | null; general_nodes: number } | null;
  concepts?: number;
}
export interface Locate { id: string; x: number; y: number; communities: string[]; layout_version: string }

async function check(res: Response): Promise<Response> {
  if (res.ok) return res;
  let code = "internal";
  let message = `${res.status} ${res.statusText}`;
  try {
    const body = (await res.json()) as { error?: { code: string; message: string } };
    if (body.error) ({ code, message } = body.error);
  } catch {
    // Not the JSON envelope (proxy or network error page): keep the status line.
  }
  throw new ApiError(res.status, code, message);
}

async function getJson<T>(path: string, params: Record<string, string | number | boolean | undefined>, signal?: AbortSignal): Promise<T> {
  const qs = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) if (v !== undefined && v !== "") qs.set(k, String(v));
  const q = qs.toString();
  const res = await check(await fetch(q ? `${path}?${q}` : path, { signal, headers: { accept: "application/json" } }));
  return (await res.json()) as T;
}

export const api = {
  health: (signal?: AbortSignal) => getJson<Health>("/api/health", {}, signal),
  search: (q: string, lang: string, limit = 12, signal?: AbortSignal) =>
    getJson<SearchResponse>("/api/search", { q, lang, limit }, signal),
  entity: (id: string, lang: string, signal?: AbortSignal) =>
    getJson<EntityResponse>(`/api/entity/${encodeURIComponent(id)}`, { lang }, signal),
  neighborhood: (id: string, lang: string, depth = 1, limit = 200, signal?: AbortSignal) =>
    getJson<NeighborhoodResponse>("/api/neighborhood", { id, lang, depth, limit }, signal),
  path: (p: { from: string; to: string; objective: string; max_hops: number; include_disputed: boolean; lang: string; weights?: string }, signal?: AbortSignal) =>
    getJson<PathResponse>("/api/path", p, signal),
  locate: (id: string, v: string, signal?: AbortSignal) => getJson<Locate>("/api/map/locate", { id, v }, signal),
  concept: (id: string, lang: string, offset = 0, limit = 50, signal?: AbortSignal) =>
    getJson<ConceptResponse>("/api/concept", { id, lang, offset, limit }, signal),
  conceptEdges: (from: string, to: string, lang: string, offset = 0, limit = 50, signal?: AbortSignal) =>
    getJson<ConceptEdgesResponse>("/api/concept/edges", { from, to, lang, offset, limit }, signal),
  async tile(layer: string, z: number, x: number, y: number, lang: string, v: string, signal?: AbortSignal): Promise<Tile> {
    const url = `/api/map/tiles/${z}/${x}/${y}?layer=${layer}&lang=${lang}&format=bin&v=${encodeURIComponent(v)}`;
    const res = await check(await fetch(url, { signal }));
    return decodeTile(await res.arrayBuffer());
  },
};
