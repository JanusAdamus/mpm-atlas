// ES/EN as first-class languages (principle 10).  Spanish first, English second.
import type { Lang } from "./vocab.ts";

type Dict = Record<string, string>;

const UI: Record<Lang, Dict> = {
  es: {
    title: "Atlas de las conexiones matemáticas",
    search: "Buscar",
    searchPlaceholder: "Buscar conceptos, teoremas, fórmulas…  ( / )",
    noResults: "Sin resultados",
    results: "{n} resultados",
    layer: "Capa",
    theme: "Tema",
    themeLight: "Claro",
    themeDark: "Oscuro",
    language: "Idioma",
    back: "Atrás",
    textView: "Vista textual",
    mapView: "Mapa",
    close: "Cerrar",
    intuition: "Intuición",
    formulation: "Formulación",
    formula: "Fórmula",
    assumptions: "Supuestos",
    connections: "Conexiones",
    sources: "Fuentes",
    license: "licencia",
    locator: "localizador",
    coverageUnknown: "Cobertura desconocida: no hay relaciones extraídas para esta entidad (no significa que esté aislada).",
    mergedFrom: "Fusionada desde",
    aliases: "También llamado",
    externalIds: "Identificadores externos",
    claimsTruncated: "Se muestran {n} afirmaciones; hay más.",
    pathFrom: "Ruta desde aquí",
    pathTo: "Ruta hasta aquí",
    compareWith: "Comparar región",
    focusRegion: "Enfocar región",
    showOnMap: "Ver en el mapa",
    path: "Ruta",
    from: "Desde",
    to: "Hasta",
    objective: "Objetivo",
    weights: "Pesos",
    maxHops: "Saltos máx.",
    includeDisputed: "Incluir disputadas",
    findPath: "Buscar ruta",
    abstained: "Sin ruta autorizada",
    exploration: "Conexión más cercana (solo exploración, no autoriza conclusión)",
    conclusion: "Conclusión",
    strength: "Fuerza",
    hypothesis: "Hipótesis",
    incomplete: "Incompleta: faltan premisas",
    missingPremises: "Premisas faltantes",
    premises: "Premisas",
    caveats: "Advertencias",
    authorizes: "Autoriza",
    routeSoFar: "Hasta aquí",
    inverse: "recorrida al revés",
    via: "vía",
    alternatives: "Alternativas",
    step: "Paso",
    compare: "Comparar regiones",
    compareHint: "Elige dos regiones: en el mapa (Mayús+clic sobre una región) o con «Comparar región» en el panel.",
    members: "Miembros",
    hiddenMembers: "Ocultos a este zoom",
    internalRelations: "Relaciones internas (agregadas)",
    central: "Miembros más centrales",
    between: "Aristas entre ambas en la vista",
    none: "ninguna",
    visibleRegions: "Regiones visibles",
    visibleEntities: "Entidades visibles",
    hiddenEdges: "Aristas no dibujadas en la vista (contadas por relación)",
    legend: "Leyenda",
    evidence: "Estado de evidencia",
    loading: "Cargando…",
    error: "Error",
    noLayout: "El mapa no está disponible: el servidor no tiene layout cargado.",
    keyboardHelp: "Teclado",
    help: "Flechas: mover · +/−: zoom · N/P: siguiente/anterior entidad visible · Enter: abrir · / : buscar · T: vista textual · Esc: cerrar · Alt+←: atrás",
    zoomIn: "Acercar",
    zoomOut: "Alejar",
    resetView: "Vista completa",
    neighborhood: "Vecindario",
    entity: "entidad",
    region: "región",
    known: "conocida",
    unknown: "desconocida",
    count: "cuenta",
    relation: "relación",
    swap: "Intercambiar",
    nodesCount: "{n} nodos",
    historyNote: "Une ideas por su nombre o su historia. No es una dependencia matemática: no autoriza concluir una idea a partir de la otra.",
    technical: "Detalles técnicos",
    advanced: "Opciones avanzadas",
    weakest: "evidencia más débil",
    fpsLabel: "FPS",
  },
  en: {
    title: "Atlas of mathematical connections",
    search: "Search",
    searchPlaceholder: "Search concepts, theorems, formulas…  ( / )",
    noResults: "No results",
    results: "{n} results",
    layer: "Layer",
    theme: "Theme",
    themeLight: "Light",
    themeDark: "Dark",
    language: "Language",
    back: "Back",
    textView: "Text view",
    mapView: "Map",
    close: "Close",
    intuition: "Intuition",
    formulation: "Formulation",
    formula: "Formula",
    assumptions: "Assumptions",
    connections: "Connections",
    sources: "Sources",
    license: "license",
    locator: "locator",
    coverageUnknown: "Coverage unknown: no relations have been extracted for this entity (it does not mean it is isolated).",
    mergedFrom: "Merged from",
    aliases: "Also known as",
    externalIds: "External identifiers",
    claimsTruncated: "Showing {n} claims; there are more.",
    pathFrom: "Path from here",
    pathTo: "Path to here",
    compareWith: "Compare region",
    focusRegion: "Focus region",
    showOnMap: "Show on map",
    path: "Path",
    from: "From",
    to: "To",
    objective: "Objective",
    weights: "Weights",
    maxHops: "Max hops",
    includeDisputed: "Include disputed",
    findPath: "Find path",
    abstained: "No authorized path",
    exploration: "Closest connection (exploration only; authorizes no conclusion)",
    conclusion: "Conclusion",
    strength: "Strength",
    hypothesis: "Hypothesis",
    incomplete: "Incomplete: premises missing",
    missingPremises: "Missing premises",
    premises: "Premises",
    caveats: "Caveats",
    authorizes: "Authorizes",
    routeSoFar: "So far",
    inverse: "walked in reverse",
    via: "via",
    alternatives: "Alternatives",
    step: "Step",
    compare: "Compare regions",
    compareHint: "Pick two regions: on the map (Shift+click a region) or with “Compare region” in the panel.",
    members: "Members",
    hiddenMembers: "Hidden at this zoom",
    internalRelations: "Internal relations (aggregated)",
    central: "Most central members",
    between: "Edges between both in view",
    none: "none",
    visibleRegions: "Visible regions",
    visibleEntities: "Visible entities",
    hiddenEdges: "Edges not drawn in view (counted per relation)",
    legend: "Legend",
    evidence: "Evidence state",
    loading: "Loading…",
    error: "Error",
    noLayout: "The map is unavailable: the server has no layout loaded.",
    keyboardHelp: "Keyboard",
    help: "Arrows: pan · +/−: zoom · N/P: next/previous visible entity · Enter: open · / : search · T: text view · Esc: close · Alt+←: back",
    zoomIn: "Zoom in",
    zoomOut: "Zoom out",
    resetView: "Full view",
    neighborhood: "Neighborhood",
    entity: "entity",
    region: "region",
    known: "known",
    unknown: "unknown",
    count: "count",
    relation: "relation",
    swap: "Swap",
    nodesCount: "{n} nodes",
    historyNote: "Links ideas through naming or history. It is not a mathematical dependency: it authorizes no conclusion from one idea to the other.",
    technical: "Technical details",
    advanced: "Advanced options",
    weakest: "weakest evidence",
    fpsLabel: "FPS",
  },
};

const RELATIONS: Record<Lang, Dict> = {
  es: {
    depends_on_logically: "depende lógicamente de",
    proves: "demuestra",
    prerequisite_for: "es prerrequisito de",
    generalizes: "generaliza",
    special_case_of: "es caso particular de",
    equivalent_to: "equivale a",
    analogous_to: "es análogo a",
    applied_in: "se aplica en",
    models: "modela",
    cites: "cita",
    influenced: "influyó en",
    part_of_area: "pertenece al área",
    formalized_as: "formalizado como",
    named_after: "lleva el nombre de",
    counterexample_to: "es contraejemplo de",
    appears_in: "aparece en",
    authored_by: "escrito por",
  },
  en: {
    depends_on_logically: "depends logically on",
    proves: "proves",
    prerequisite_for: "is a prerequisite for",
    generalizes: "generalizes",
    special_case_of: "is a special case of",
    equivalent_to: "is equivalent to",
    analogous_to: "is analogous to",
    applied_in: "is applied in",
    models: "models",
    cites: "cites",
    influenced: "influenced",
    part_of_area: "is part of area",
    formalized_as: "is formalized as",
    named_after: "is named after",
    counterexample_to: "is a counterexample to",
    appears_in: "appears in",
    authored_by: "authored by",
  },
};

const STATES: Record<Lang, Dict> = {
  es: {
    extracted: "extraída",
    model_proposed: "propuesta por modelo",
    editorially_reviewed: "revisada editorialmente",
    formally_linked: "vinculada formalmente",
    disputed: "disputada",
    superseded: "reemplazada",
    retracted: "retractada",
  },
  en: {
    extracted: "extracted",
    model_proposed: "model proposed",
    editorially_reviewed: "editorially reviewed",
    formally_linked: "formally linked",
    disputed: "disputed",
    superseded: "superseded",
    retracted: "retracted",
  },
};

const TYPES: Record<Lang, Dict> = {
  es: {
    concept: "concepto", definition: "definición", theory: "teoría", statement: "enunciado", proof: "demostración",
    conjecture: "conjetura", counterexample: "contraejemplo", algorithm: "algoritmo", model: "modelo",
    problem: "problema", phenomenon: "fenómeno", application: "aplicación", publication: "publicación",
    source_fragment: "fragmento de fuente", person: "persona", resource: "recurso", experiment: "experimento",
    area: "área",
  },
  en: {
    concept: "concept", definition: "definition", theory: "theory", statement: "statement", proof: "proof",
    conjecture: "conjecture", counterexample: "counterexample", algorithm: "algorithm", model: "model",
    problem: "problem", phenomenon: "phenomenon", application: "application", publication: "publication",
    source_fragment: "source fragment", person: "person", resource: "resource", experiment: "experiment",
    area: "area",
  },
};

const PROJECTIONS: Record<Lang, Dict> = {
  es: { structure: "Estructura", proof: "Demostración", learning: "Aprendizaje", application: "Aplicación", history: "Historia", biblio: "Bibliografía" },
  en: { structure: "Structure", proof: "Proof", learning: "Learning", application: "Application", history: "History", biblio: "Bibliography" },
};

const OBJECTIVES: Record<Lang, Dict> = {
  es: {
    best_supported: "Mejor respaldada", fewest_new: "Menos conceptos nuevos", most_intuitive: "Más intuitiva",
    most_formal: "Más formal", most_diverse: "Más diversa", shortest: "Más corta",
  },
  en: {
    best_supported: "Best supported", fewest_new: "Fewest new concepts", most_intuitive: "Most intuitive",
    most_formal: "Most formal", most_diverse: "Most diverse", shortest: "Shortest",
  },
};

/** Reader-facing class of a path conclusion (`Conclusion.category` from atlas-paths). */
const CATEGORIES: Record<Lang, Dict> = {
  es: {
    mathematical: "Matemática", structural: "Estructural", application: "Aplicación", pedagogical: "Pedagógica",
    candidate_hypothesis: "Hipótesis candidata", historical: "Conexión histórica", documentary: "Documental", none: "Sin conclusión",
  },
  en: {
    mathematical: "Mathematical", structural: "Structural", application: "Application", pedagogical: "Pedagogical",
    candidate_hypothesis: "Candidate hypothesis", historical: "Historical connection", documentary: "Documentary", none: "No conclusion",
  },
};

let current: Lang = "es";

export const getLang = (): Lang => current;
export function setLang(l: Lang): void {
  current = l;
  document.documentElement.lang = l;
}

/** UI string, with `{name}` placeholders filled from `vars`. */
export function t(key: string, vars: Record<string, string | number> = {}): string {
  const s = UI[current][key] ?? UI.es[key] ?? key;
  return s.replace(/\{(\w+)\}/g, (_, k: string) => String(vars[k] ?? ""));
}

export const relLabel = (rel: string): string => RELATIONS[current][rel] ?? rel.replaceAll("_", " ");
export const stateLabel = (s: string): string => STATES[current][s] ?? s;
export const typeLabel = (ty: string): string => TYPES[current][ty] ?? ty;
export const projectionLabel = (p: string): string => PROJECTIONS[current][p] ?? p;
export const objectiveLabel = (o: string): string => OBJECTIVES[current][o] ?? o;
export const categoryLabel = (c: string): string => CATEGORIES[current][c] ?? c.replaceAll("_", " ");
export const OBJECTIVE_NAMES = Object.keys(OBJECTIVES.es);

/** Every UI key exists in both languages (checked by i18n.test.ts). */
export const uiKeys = (l: Lang): string[] => Object.keys(UI[l]);
export const relationKeys = (l: Lang): string[] => Object.keys(RELATIONS[l]);
export const stateKeys = (l: Lang): string[] => Object.keys(STATES[l]);
export const typeKeys = (l: Lang): string[] => Object.keys(TYPES[l]);
