// Vocabulary codes shared with Rust and Python: shapes/vocab.json is generated from
// atlas/model.py, and a binary-tile code is the index in these arrays.
import vocab from "../../shapes/vocab.json" with { type: "json" };

export type Lang = "es" | "en";

export const ENTITY_TYPES: readonly string[] = vocab.entity_types;
export const RELATION_TYPES: readonly string[] = vocab.relation_types;
export const EVIDENCE_STATES: readonly string[] = vocab.evidence_states;
export const CURRENT_STATES: readonly string[] = vocab.current_states;
export const PROJECTIONS: readonly string[] = vocab.projections;

interface RelationInfo {
  projection: string;
  symmetric: boolean;
  transitive: boolean;
  inverse: string | null;
  allows_premises: boolean;
  chain_authorizes: string;
}

const RELATIONS = vocab.relations as Record<string, RelationInfo>;

export function relationInfo(rel: string): RelationInfo | undefined {
  return RELATIONS[rel];
}

export const relationName = (code: number): string => RELATION_TYPES[code] ?? `rel#${code}`;
export const stateName = (code: number): string => EVIDENCE_STATES[code] ?? `state#${code}`;
export const typeName = (code: number): string => ENTITY_TYPES[code] ?? "";
