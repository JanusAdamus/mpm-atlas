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

/** Ids of technical records (Mathlib declarations): hidden in the explore mode. */
export const isCodeId = (id: string): boolean => id.startsWith("mathlib:");

/** What must never show in the explore mode (docs/plans/2026-10-19-ronda-4-diseno.md, rule
 *  «Jerga en modo Explorar»): ids, source jargon and internal counts.  «derivada» (the
 *  concept) is allowed; «derivado» (a derived concept) is not. */
export const JARGON: readonly RegExp[] = [
  /wd:/, /mathlib:/, /Mathlib/i, /\bderivados?\b/i, /docstring/i, /model_proposed/,
  /testigo/i, /afirmaci[oó]n/i, /declaraci[oó]n/i, /\bR7\b/, /\bQ\d+\b/, /\d\s*nodos\b/i, /\bMiembros\b/,
];

/** The jargon found in a visible text (empty when clean). */
export function jargonIn(text: string): string[] {
  return JARGON.flatMap((re) => {
    const m = re.exec(text);
    return m ? [m[0]] : [];
  });
}
