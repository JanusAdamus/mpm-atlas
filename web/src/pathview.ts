// Path view: typed routes with per-step justification and what the whole route
// authorizes.  Round 4: every route is first a step-by-step story (one hop per card, a
// plain verb from the side walked, its evidence seal and what it lets you conclude), moved
// with buttons, arrows and keys.  The technical mode adds today's detail below it
// (strength in a fold, weakest evidence, premises, technical backing).
import { api, type Conclusion, type ConceptStep, type FoundPath, type PathResponse, type PathStep, type RouteStop } from "./api.ts";
import { h } from "./dom.ts";
import { badge, historicalBadge, sourceTitle } from "./evidence.ts";
import { authorizesPlain, categoryLabel, getLang, hopSentence, isExplore, OBJECTIVE_NAMES, objectiveLabel, relLabel, t } from "./i18n.ts";
import { searchBox } from "./search.ts";
import { jargonIn } from "./vocab.ts";

export interface PathEnd {
  id: string;
  label: string;
}

export interface PathQueryState {
  from: PathEnd | null;
  to: PathEnd | null;
  objective: string;
  maxHops: number;
  includeDisputed: boolean;
  weights: string;
  /** Step shown in the step-by-step card (0-based). */
  step?: number;
}

export interface PathViewActions {
  open(id: string): void;
  openConcept(id: string): void;
  /** Draw the route on the map.  `historical` means only a historical route (its own stroke);
   *  every other hop is drawn with its evidence state, exploration included. */
  showPath(steps: RouteStop[], historical: boolean): void;
  changed(q: PathQueryState): void;
  /** The reader moved to step `i` of `steps` (the map follows). */
  step?(i: number, steps: RouteStop[]): void;
  /** Move focus to the step-by-step card once the answer arrives (guided doors). */
  focusResult?: boolean;
}

const MAX_STRENGTH = 5;

/** A historical route is shown as such, never as a mathematical dependency.  Servers
 *  before `category` existed only had `kind`. */
export function isHistorical(c: Conclusion): boolean {
  return c.category === "historical" || c.kind === "historical";
}

type ConceptRoute = NonNullable<NonNullable<PathResponse["concepts"]>["route"]>;

/** The concept-level route of an answer, when there is one to show. */
export function conceptRoute(r: PathResponse): ConceptRoute | null {
  const c = r.concepts;
  return c && !c.abstained && c.route?.steps.length ? c.route : null;
}

/** What the map draws for an answer: the declaration-level route when there is one, else
 *  the concept route, else the exploration.  `historical` only for a historical route: an
 *  exploration is not history (C4-02).  Never a stale route. */
export function mapRoute(r: PathResponse): { steps: RouteStop[]; historical: boolean } {
  if (!r.abstained && r.paths[0]) return { steps: r.paths[0].steps, historical: isHistorical(r.paths[0].conclusion) };
  const c = conceptRoute(r);
  if (c) return { steps: c.steps, historical: isHistorical(c.conclusion) };
  return { steps: r.exploration?.steps ?? [], historical: false };
}

function categoryOf(c: Conclusion): string | null {
  if (c.category) return c.category;
  if (c.kind === "historical") return "historical";
  return c.mathematical ? "mathematical" : null;
}

function strengthMeter(n: number): HTMLElement {
  const label = `${t("strength")} ${n}/${MAX_STRENGTH}`;
  const pips = h("span", { class: "pips", role: "img", "aria-label": label },
    Array.from({ length: MAX_STRENGTH }, (_, i) => h("i", { class: i < n ? "on" : "" })));
  return h("span", { class: "strength" }, h("span", { class: "muted" }, t("strength")), pips, h("span", { "aria-hidden": "true" }, `${n}/${MAX_STRENGTH}`));
}

function hopItem(s: PathStep, i: number, link: (id: string, label: string) => HTMLElement): HTMLElement {
  const premises = s.premises.length
    ? h("div", { class: "premises" }, `${t("premises")}: `,
      ...s.premises.map((p) => {
        const missing = s.premises_missing.includes(p);
        const how = s.premises_satisfied.find((x) => x.premise === p);
        return h("span", { class: missing ? "premise missing" : "premise ok", title: how ? `${how.how}: ${how.claims.join(", ")}` : "" },
          missing ? "✗ " : "✓ ", link(p, p), how ? ` (${how.how})` : "", " ");
      }))
    : null;
  const sources = s.sources.map((src) => `${src.source_id} · ${src.locator} (${src.license})`).join("; ");
  return h("li", { class: "hop" },
    h("div", { class: "rel" },
      h("span", { class: "sr-only" }, `${t("step")} ${i + 1}: `),
      h("strong", {}, `${relLabel(s.relation)} →`),
      s.direction === "inverse" ? h("span", { class: "tag", title: t("inverse") }, `↺ ${t("inverse")}`) : null,
      badge(s.evidence_state)),
    h("div", { class: "auth" }, h("b", {}, `${t("authorizes")}: `), s.authorizes),
    s.assumptions.length ? h("div", { class: "assumptions" }, `${t("assumptions")}: ${s.assumptions.join("; ")}`) : null,
    premises,
    s.via.length ? h("div", { class: "muted" }, `${t("via")}: `, ...s.via.map((v) => link(v, v))) : null,
    s.validity_domain ? h("div", { class: "muted" }, `⟂ ${s.validity_domain}`) : null,
    s.alternatives.length ? h("div", { class: "muted" }, `${t("alternatives")}: ${s.alternatives.join(", ")}`) : null,
    h("div", { class: "prov", title: `${s.claim_id} · ${sources}` }, `${s.claim_id} · ${sources}`),
  );
}

type Link = (id: string, label: string) => HTMLElement;

/** A route as stops (numbered like the map markers) joined by justified hops. */
function stops<S extends RouteStop>(steps: S[], dashed: boolean, open: (id: string) => void, hop: (s: S, i: number, link: Link) => HTMLElement): HTMLElement {
  const link: Link = (id, label) => h("button", { class: "link", type: "button", onclick: () => open(id) }, label || id);
  const stop = (n: number, id: string, label: string) => h("li", { class: "stop" }, h("span", { class: "n", "aria-hidden": "true" }, String(n)), link(id, label));
  const items: HTMLElement[] = [];
  steps.forEach((s, i) => {
    if (i === 0) items.push(stop(1, s.from, s.from_label));
    items.push(hop(s, i, link));
    items.push(stop(i + 2, s.to, s.to_label));
  });
  return h("ol", { class: dashed ? "route historical" : "route" }, items);
}

const routeList = (p: FoundPath, dashed: boolean, actions: PathViewActions): HTMLElement => stops(p.steps, dashed, actions.open, hopItem);

let drillSeq = 0;

/** A concept step: what it aggregates, its witness claim, and a button that unfolds the
 *  witness as a declaration-level route (and draws that route on the map). */
function conceptHop(s: ConceptStep, i: number, actions: PathViewActions): HTMLElement {
  const sources = s.witness.sources.map((src) => `${src.source_id} · ${src.locator} (${src.license})`).join("; ");
  const drill = s.drill_down;
  const box = h("div", { class: "drill", id: `drill-${++drillSeq}`, hidden: true });
  const toggle = drill
    ? h("button", { type: "button", class: "mini drill-toggle", "aria-expanded": "false", "aria-controls": box.id }, t("drillDown"))
    : null;
  toggle?.addEventListener("click", () => {
    const open = box.hidden;
    box.hidden = !open;
    toggle.setAttribute("aria-expanded", String(open));
    toggle.textContent = t(open ? "drillUp" : "drillDown");
    if (!open || !drill) return;
    const dashed = isHistorical(drill.conclusion);
    if (!box.childElementCount) {
      box.append(
        h("p", { class: "muted small" }, t("drillNote", { a: s.witness.from_label, b: s.witness.to_label })),
        ...(hasMissingPremises(drill) ? [h("p", { class: "notice crosses", role: "note" }, t("crossesNote"))] : []),
        routeList(drill, dashed, actions),
        h("button", { type: "button", class: "mini", onclick: () => actions.showPath(drill.steps, dashed) }, t("showOnMap")));
    }
    actions.showPath(drill.steps, dashed);
  });
  return h("li", { class: "hop" },
    h("div", { class: "rel" },
      h("span", { class: "sr-only" }, `${t("step")} ${i + 1}: `),
      h("strong", {}, `${relLabel(s.relation)} →`),
      s.direction === "inverse" ? h("span", { class: "tag", title: t("inverse") }, `↺ ${t("inverse")}`) : null,
      badge(s.evidence_state)),
    h("div", { class: "auth" }, h("b", {}, `${t("authorizes")}: `), s.authorizes),
    h("div", { class: "muted" }, t("declarationClaims", { n: s.declaration_claims.toLocaleString() })),
    h("div", { class: "prov", title: `${s.witness.claim_id} · ${sources}` }, `${t("witness")}: ${s.witness.claim_id} · ${sources}`),
    toggle ? h("div", { class: "row" }, toggle) : h("div", { class: "muted small" }, t("noDrill")),
    box,
  );
}

/** Whether a declaration-level route leaves some premise unprovided (drawn with a cross). */
export const hasMissingPremises = (p: FoundPath): boolean => p.steps.some((s) => s.premises_missing.length > 0);

/** The strength scale is not a degree of certainty: it goes in a fold with what it measures
 *  (atlas-paths `CompositionTable::strength`, scale of shapes/composition.json). */
function strengthDetails(n: number): HTMLElement {
  return h("details", { class: "strength-details" },
    h("summary", { class: "small" }, t("strengthWhat")),
    h("div", { class: "row" }, strengthMeter(n)),
    h("p", { class: "muted small" }, t("strengthMeans")));
}

/** `candidate`: there is no validated (declaration-level) route above it, so the concept
 *  route is presented as a candidate pending review, never with its strength up front. */
function conceptBlock(route: ConceptRoute, actions: PathViewActions, candidate: boolean): HTMLElement {
  const c = route.conclusion;
  const dashed = isHistorical(c);
  const title = candidate ? t("conceptCandidate") : t("conceptRoute");
  const cls = ["conclusion", dashed ? "historical" : "", candidate ? "candidate" : ""].filter(Boolean).join(" ");
  return h("section", { class: candidate ? "path concept-route candidate" : "path concept-route", "aria-label": title },
    h("h3", {}, title),
    h("div", { class: cls },
      h("p", { class: "claimline" }, c.authorizes),
      h("div", { class: "row" }, h("span", { class: "muted" }, t("weakest")), badge(c.weakest_evidence)),
      c.caveats.length ? h("ul", { class: "caveats small", "aria-label": t("caveats") }, c.caveats.map((x) => h("li", {}, x))) : null,
      strengthDetails(c.strength),
      h("div", { class: "row" }, h("button", { type: "button", onclick: () => actions.showPath(route.steps, dashed) }, t("showOnMap")))),
    stops(route.steps, dashed, actions.openConcept, (s, i) => conceptHop(s, i, actions)),
  );
}

function pathBlock(p: FoundPath, title: string, actions: PathViewActions, exploration = false): HTMLElement {
  const c = p.conclusion;
  const historical = isHistorical(c);
  const cat = categoryOf(c);
  const weak = !historical && (exploration || c.strength === 0);
  const rest = historical ? c.caveats.slice(1) : c.caveats;
  const cls = ["conclusion", historical ? "historical" : "", weak ? "weak" : ""].filter(Boolean).join(" ");
  return h("section", { class: "path", "aria-label": title },
    h("h3", {}, title),
    h("div", { class: cls },
      cat ? h("div", { class: "category" }, categoryLabel(cat)) : null,
      h("p", { class: "claimline" }, c.authorizes),
      // Historical routes lead with the server's own caveat (what the route does not mean).
      historical ? h("p", { class: "history-note" }, c.caveats[0] ?? t("historyNote")) : null,
      h("div", { class: "row" }, strengthMeter(c.strength), h("span", { class: "muted", "aria-hidden": "true" }, "·"),
        h("span", { class: "muted" }, t("weakest")), badge(c.weakest_evidence),
        c.is_hypothesis ? h("span", { class: "tag warn" }, t("hypothesis")) : null,
        c.complete ? null : h("span", { class: "tag warn" }, t("incomplete"))),
      c.missing_premises.length ? h("div", { class: "row" }, `${t("missingPremises")}: ${c.missing_premises.join(", ")}`) : null,
      c.assumptions.length ? h("div", { class: "assumptions" }, `${t("assumptions")}: ${c.assumptions.join("; ")}`) : null,
      rest.length ? h("ul", { class: "caveats small", "aria-label": t("caveats") }, rest.map((x) => h("li", {}, x))) : null,
      h("div", { class: "row" }, h("button", { type: "button", onclick: () => actions.showPath(p.steps, historical) }, t("showOnMap"))),
    ),
    routeList(p, historical || exploration, actions),
  );
}

function techDetails(r: PathResponse): HTMLElement {
  const fmt = (o: Record<string, number>, d?: number) => Object.entries(o).map(([k, v]) => `${k} ${d === undefined ? v : v.toFixed(d)}`).join(", ");
  const found = [...r.paths, ...(r.exploration ? [r.exploration] : [])];
  return h("details", { class: "tech" },
    h("summary", {}, t("technical")),
    h("p", {}, `${r.stats.algorithm} · ${r.stats.expanded} ${t("expandedShort")} · ${(r.stats.micros / 1000).toFixed(1)} ms`),
    h("p", {}, `${t("weights")}: ${fmt(r.weights)}`),
    ...found.map((p, i) => h("p", {}, `#${i + 1} ${p.conclusion.kind} · ${t("score")} ${p.score.toFixed(3)} · ${fmt(p.cost_terms, 2)}`)),
  );
}

/** A stop name a reader can read: derived concepts arrive as «group › conj (Mathlib, derivado)»;
 *  the explore mode drops that tag, and a name that is itself jargon reads as «(sin nombre)». */
export function readerLabel(label: string): string {
  if (!isExplore()) return label || t("noName");
  // A Wikipedia article is a document, not a concept: named as such, with the title's language
  // when it is not the reader's (the registry has no link between language versions).
  const art = /^Wikipedia \(([a-z-]{2,12})\): (.+)$/.exec(label);
  if (art) {
    const lang = t(`lang_${art[1]}`);
    return art[1] === getLang() ? t("article", { title: art[2] }) : t("articleIn", { lang: lang.startsWith("lang_") ? art[1] : lang, title: art[2] });
  }
  const clean = label.replace(/\s*\((?:Mathlib|mathlib)[^)]*\)\s*$/, "").trim();
  return clean && !jargonIn(clean).length ? clean : t("noName");
}

/** One hop of any route level: declaration steps and concept steps both carry these. */
export type Hop = RouteStop & Pick<PathStep, "relation" | "direction" | "evidence_state"> & Partial<Pick<PathStep, "sources">>;

/** «Según Wikipedia, »: an unreviewed hop is said in its sources' voice (C4-06). */
export function hopLead(s: Hop): string {
  const reviewed = s.evidence_state === "editorially_reviewed" || s.evidence_state === "formally_linked";
  const names = [...new Set((s.sources ?? []).map((x) => sourceTitle(x.source_id)))];
  return reviewed || !names.length ? "" : t("accordingTo", { src: new Intl.ListFormat(getLang(), { type: "conjunction" }).format(names) });
}

/** The route the step-by-step card walks: the one the map draws (mapRoute), with its title. */
export function walkedRoute(r: PathResponse): { steps: Hop[]; historical: boolean; dashed: boolean; title: string; candidate: boolean } | null {
  if (!r.abstained && r.paths[0]) {
    const c = r.paths[0].conclusion;
    return { steps: r.paths[0].steps, historical: isHistorical(c), dashed: isHistorical(c), title: categoryLabel(categoryOf(c) ?? "none"), candidate: false };
  }
  const concept = conceptRoute(r);
  if (concept) return { steps: concept.steps, historical: isHistorical(concept.conclusion), dashed: isHistorical(concept.conclusion), title: t("conceptCandidate"), candidate: true };
  if (r.exploration?.steps.length) return { steps: r.exploration.steps, historical: false, dashed: false, title: t("exploration"), candidate: false };
  return null;
}

/** Step-by-step card: numbered stops (the map's numbers), one hop per card, moved with the
 *  buttons, ←/→ (Home/End) on the card, or by picking a stop. */
export function stepper(route: NonNullable<ReturnType<typeof walkedRoute>>, start: number, actions: Pick<PathViewActions, "open" | "step">): HTMLElement {
  const { steps } = route;
  const n = steps.length;
  let at = Math.max(0, Math.min(n - 1, start));
  const card = h("div", { class: "step-card", "aria-live": "polite" });
  const prev = h("button", { type: "button", class: "step-prev", onclick: () => go(at - 1) }, `← ${t("prevStep")}`);
  const next = h("button", { type: "button", class: "step-next primary", onclick: () => go(at + 1) }, `${t("nextStep")} →`);
  const stopIds = [steps[0].from, ...steps.map((s) => s.to)];
  const stopLabels = [steps[0].from_label, ...steps.map((s) => s.to_label)].map(readerLabel);
  const stopBtns = stopIds.map((_, k) => h("button", { type: "button", class: "stop-btn", onclick: () => go(Math.min(k, n - 1)) },
    h("span", { class: "n" }, String(k + 1)), stopLabels[k]));
  const stops = h("ol", { class: route.dashed ? "stops dashed" : "stops", "aria-label": t("routeStops") },
    stopBtns.map((b, k) => h("li", { style: `--i:${k}` }, b)));
  const link = (id: string, label: string): HTMLElement => h("button", { type: "button", class: "link", onclick: () => actions.open(id) }, readerLabel(label));
  const draw = (): void => {
    const s = steps[at];
    const inverse = s.direction === "inverse";
    card.replaceChildren(
      h("p", { class: "step-pos" }, t("stepOf", { i: at + 1, n })),
      h("p", { class: "step-ends" }, h("span", { class: "n", "aria-hidden": "true" }, String(at + 1)), link(s.from, s.from_label),
        h("span", { class: "arrow", "aria-hidden": "true" }, " → "), h("span", { class: "n", "aria-hidden": "true" }, String(at + 2)), link(s.to, s.to_label)),
      h("p", { class: "step-sentence" }, hopLead(s), hopSentence(s.relation, inverse, readerLabel(s.from_label), readerLabel(s.to_label))),
      h("p", { class: "seal" }, badge(s.evidence_state), route.historical ? historicalBadge() : null),
      h("div", { class: "may" }, h("b", {}, `${t("mayConclude")}: `), authorizesPlain(s.relation),
        s.evidence_state === "model_proposed" ? h("p", { class: "muted small" }, t("candidateStep")) : null),
    );
    stopBtns.forEach((b, k) => (k === at || k === at + 1 ? b.setAttribute("aria-current", "step") : b.removeAttribute("aria-current")));
    prev.disabled = at === 0;
    next.disabled = at === n - 1;
  };
  const go = (i: number): void => {
    const to = Math.max(0, Math.min(n - 1, i));
    if (to === at && card.childElementCount) return;
    at = to;
    draw();
    actions.step?.(at, steps);
  };
  const root = h("section", { class: route.candidate ? "stepper candidate" : "stepper", tabindex: 0, "aria-label": t("stepByStep"),
    onkeydown: (e: Event) => {
      const k = (e as KeyboardEvent).key;
      const to = k === "ArrowRight" ? at + 1 : k === "ArrowLeft" ? at - 1 : k === "Home" ? 0 : k === "End" ? n - 1 : null;
      if (to === null || (e.target as HTMLElement).tagName === "INPUT") return;
      e.preventDefault();
      go(to);
    } },
    h("h3", {}, route.title),
    stops, card,
    h("div", { class: "step-nav" }, prev, next, h("span", { class: "muted small" }, t("stepKeys"))));
  draw();
  return root;
}

export function renderPathResult(r: PathResponse, actions: PathViewActions, start = 0): HTMLElement {
  const root = h("div", { class: "path-result" });
  const concept = conceptRoute(r);
  const walked = walkedRoute(r);
  if (r.abstained) {
    // No validated route: said on its own, apart from the candidate and documentary routes below.
    // The server's reason quotes the strength scale: technical mode only.
    root.append(h("div", { class: "notice no-route", role: "note" },
      h("p", { class: "lead" }, h("strong", {}, t("abstained"))),
      r.reason && !isExplore() ? h("p", { class: "muted small" }, r.reason) : null));
  }
  if (walked) root.append(stepper(walked, start, actions));
  // The explore mode stops at the story; the technical mode keeps today's detail below it.
  if (isExplore()) return root;
  if (r.abstained) {
    // Concept-first: with no declaration-level answer, the concept route is shown as a candidate.
    if (concept) root.append(conceptBlock(concept, actions, true));
    if (r.exploration) root.append(pathBlock(r.exploration, t("exploration"), actions, true));
  } else {
    r.paths.forEach((p, i) => root.append(pathBlock(p, r.paths.length > 1 ? `${t("path")} ${i + 1}` : t("conclusion"), actions)));
    if (concept) root.append(conceptBlock(concept, actions, false));
  }
  root.append(techDetails(r));
  return root;
}

/** An end with its label in the active language (a URL may carry only the id, or a label in
 *  the other language).  On any failure the end is kept as it was. */
export async function localizedEnd(end: PathEnd, fetchLabel: (id: string) => Promise<string>): Promise<PathEnd> {
  try {
    const label = await fetchLabel(end.id);
    return label ? { id: end.id, label } : end;
  } catch {
    return end;
  }
}

const fetchLabel = (id: string): Promise<string> => api.entity(id, getLang(), undefined, { limit: 1 }).then((r) => r.label);

export function pathView(state: PathQueryState, actions: PathViewActions): HTMLElement {
  const q = { ...state };
  const result = h("div", {});
  const announce = h("p", { class: "sr-only", role: "status", "aria-live": "polite" });
  const fromBox = searchBox({ label: t("from"), placeholder: t("from"), onPick: (hit) => { q.from = { id: hit.id, label: hit.label }; fromBox.input.title = hit.id; actions.changed({ ...q }); } });
  const toBox = searchBox({ label: t("to"), placeholder: t("to"), onPick: (hit) => { q.to = { id: hit.id, label: hit.label }; toBox.input.title = hit.id; actions.changed({ ...q }); } });
  // The fields show labels, never raw ids: the id goes in the title, the label is fetched in the active language.
  const showEnd = (box: typeof fromBox, end: PathEnd | null, set: (e: PathEnd) => void, current: () => PathEnd | null): void => {
    if (!end) return;
    box.setValue(end.label === end.id ? "" : end.label);
    box.input.title = end.id;
    void localizedEnd(end, fetchLabel).then((e) => {
      if (current()?.id !== e.id) return; // the reader picked another end meanwhile
      set(e);
      box.setValue(e.label);
      actions.changed({ ...q });
    });
  };
  showEnd(fromBox, q.from, (e) => { q.from = e; }, () => q.from);
  showEnd(toBox, q.to, (e) => { q.to = e; }, () => q.to);
  const objective = h("select", { id: "path-objective" }, OBJECTIVE_NAMES.map((o) => h("option", { value: o, selected: o === q.objective }, objectiveLabel(o))));
  const hops = h("input", { id: "path-hops", type: "number", min: 1, max: 10, value: q.maxHops });
  const disputed = h("input", { id: "path-disputed", type: "checkbox", checked: q.includeDisputed });
  const weights = h("input", { id: "path-weights", type: "text", value: q.weights, placeholder: "evidence:1,length:0.2", spellcheck: "false" });
  let ctrl: AbortController | null = null;
  const run = async (): Promise<void> => {
    q.objective = objective.value;
    q.maxHops = Math.max(1, Math.min(10, Number(hops.value) || 6));
    q.includeDisputed = disputed.checked;
    q.weights = weights.value.trim();
    actions.changed({ ...q });
    if (!q.from || !q.to) return;
    ctrl?.abort();
    ctrl = new AbortController();
    result.replaceChildren(h("p", { class: "muted" }, t("loading")));
    try {
      const r = await api.path({ from: q.from.id, to: q.to.id, objective: q.objective, max_hops: q.maxHops, include_disputed: q.includeDisputed, lang: getLang(), weights: q.weights || undefined, concepts: true, drill_down: true }, ctrl.signal);
      const onStep = (i: number, steps: RouteStop[]): void => {
        q.step = i;
        actions.changed({ ...q });
        actions.step?.(i, steps);
      };
      result.replaceChildren(renderPathResult(r, { ...actions, step: onStep }, q.step ?? 0));
      const walked = walkedRoute(r);
      announce.textContent = walked ? t("routeReady", { n: walked.steps.length }) : t("abstained");
      // The map always shows this answer (or nothing): never a stale route from an earlier query.
      // Each hop with its own evidence stroke; only a historical route gets the historical one.
      const m = mapRoute(r);
      actions.showPath(m.steps, m.historical);
      if (walked) actions.step?.(Math.min(q.step ?? 0, walked.steps.length - 1), walked.steps);
      // The answer can arrive before the panel has entered (it waits for the previous one to leave).
      const focusCard = (tries: number): void => {
        const card = result.querySelector<HTMLElement>(".stepper");
        if (card?.isConnected) card.focus();
        else if (card && tries > 0) setTimeout(() => focusCard(tries - 1), 50);
      };
      if (actions.focusResult) focusCard(20);
    } catch (err) {
      if ((err as Error).name === "AbortError") return;
      result.replaceChildren(h("p", { class: "notice error", role: "alert" }, `${t("error")}: ${(err as Error).message}`));
    }
  };
  const swap = (): void => {
    [q.from, q.to] = [q.to, q.from];
    q.step = 0;
    fromBox.setValue(q.from?.label ?? "");
    toBox.setValue(q.to?.label ?? "");
    fromBox.input.title = q.from?.id ?? "";
    toBox.input.title = q.to?.id ?? "";
    void run();
  };
  const form = h("form", { class: "path-form", onsubmit: (e: Event) => { e.preventDefault(); q.step = 0; void run(); } },
    h("div", { class: "ends" },
      h("div", { class: "ends-col" },
        h("div", { class: "row" }, h("span", { class: "lbl" }, t("from")), fromBox.root),
        h("div", { class: "row" }, h("span", { class: "lbl" }, t("to")), toBox.root)),
      h("button", { type: "button", class: "swap", onclick: swap, title: t("swap"), "aria-label": t("swap") }, "⇅")),
    h("div", { class: "row" }, h("label", { class: "lbl", for: "path-objective" }, t("objective")), objective),
    isExplore() ? null : h("details", { open: q.includeDisputed || !!q.weights || q.maxHops !== 6 },
      h("summary", {}, t("advanced")),
      h("div", { class: "row wrap" },
        h("label", { for: "path-hops" }, t("maxHops")), hops,
        h("label", {}, disputed, ` ${t("includeDisputed")}`)),
      h("div", { class: "row" }, h("label", { for: "path-weights" }, t("weights")), weights)),
    h("div", { class: "submit" }, h("button", { type: "submit", class: "primary" }, t("findPath"))),
  );
  if (q.from && q.to) void run();
  return h("section", { class: "pathview", "aria-label": t("path") }, h("h2", {}, t("path")), form, announce, result);
}
