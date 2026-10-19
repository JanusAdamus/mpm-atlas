// Path view: typed routes with per-step justification and what the whole route
// authorizes (strength 0..5, weakest evidence, hypothesis, missing premises).
import { api, type Conclusion, type ConceptStep, type FoundPath, type PathResponse, type PathStep, type RouteStop } from "./api.ts";
import { h } from "./dom.ts";
import { badge } from "./evidence.ts";
import { categoryLabel, getLang, OBJECTIVE_NAMES, objectiveLabel, relLabel, t } from "./i18n.ts";
import { searchBox } from "./search.ts";

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
}

export interface PathViewActions {
  open(id: string): void;
  openConcept(id: string): void;
  /** Draw the route on the map; `historical` routes are drawn dashed, never as a dependency. */
  showPath(steps: RouteStop[], historical: boolean): void;
  changed(q: PathQueryState): void;
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
 *  the concept route, else the exploration (dashed).  Never a stale route. */
export function mapRoute(r: PathResponse): { steps: RouteStop[]; dashed: boolean } {
  if (!r.abstained && r.paths[0]) return { steps: r.paths[0].steps, dashed: isHistorical(r.paths[0].conclusion) };
  const c = conceptRoute(r);
  if (c) return { steps: c.steps, dashed: isHistorical(c.conclusion) };
  return { steps: r.exploration?.steps ?? [], dashed: true };
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

function conceptBlock(route: ConceptRoute, actions: PathViewActions): HTMLElement {
  const c = route.conclusion;
  const dashed = isHistorical(c);
  return h("section", { class: "path concept-route", "aria-label": t("conceptRoute") },
    h("h3", {}, t("conceptRoute")),
    h("div", { class: dashed ? "conclusion historical" : "conclusion" },
      h("p", { class: "claimline" }, c.authorizes),
      h("div", { class: "row" }, strengthMeter(c.strength), h("span", { class: "muted", "aria-hidden": "true" }, "·"),
        h("span", { class: "muted" }, t("weakest")), badge(c.weakest_evidence)),
      c.caveats.length ? h("ul", { class: "caveats small", "aria-label": t("caveats") }, c.caveats.map((x) => h("li", {}, x))) : null,
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
      h("div", { class: "row" }, h("button", { type: "button", onclick: () => actions.showPath(p.steps, historical || exploration) }, t("showOnMap"))),
    ),
    routeList(p, historical || exploration, actions),
  );
}

function techDetails(r: PathResponse): HTMLElement {
  const fmt = (o: Record<string, number>, d?: number) => Object.entries(o).map(([k, v]) => `${k} ${d === undefined ? v : v.toFixed(d)}`).join(", ");
  const found = [...r.paths, ...(r.exploration ? [r.exploration] : [])];
  return h("details", { class: "tech" },
    h("summary", {}, t("technical")),
    h("p", {}, `${r.stats.algorithm} · ${r.stats.expanded} exp · ${(r.stats.micros / 1000).toFixed(1)} ms`),
    h("p", {}, `${t("weights")}: ${fmt(r.weights)}`),
    ...found.map((p, i) => h("p", {}, `#${i + 1} ${p.conclusion.kind} · score ${p.score.toFixed(3)} · ${fmt(p.cost_terms, 2)}`)),
  );
}

export function renderPathResult(r: PathResponse, actions: PathViewActions): HTMLElement {
  const root = h("div", { class: "path-result", "aria-live": "polite" });
  const concept = conceptRoute(r);
  if (r.abstained) {
    root.append(h("div", { class: "notice", role: "note" }, h("strong", {}, `${t("abstained")}. `), r.reason ?? r.abstention ?? ""));
    // Concept-first: with no declaration-level answer, the concept route is the answer.
    if (concept) root.append(conceptBlock(concept, actions));
    if (r.exploration) root.append(pathBlock(r.exploration, t("exploration"), actions, true));
  } else {
    r.paths.forEach((p, i) => root.append(pathBlock(p, r.paths.length > 1 ? `${t("path")} ${i + 1}` : t("conclusion"), actions)));
    if (concept) root.append(conceptBlock(concept, actions));
  }
  root.append(techDetails(r));
  return root;
}

export function pathView(state: PathQueryState, actions: PathViewActions): HTMLElement {
  const q = { ...state };
  const result = h("div", {});
  const fromBox = searchBox({ label: t("from"), placeholder: t("from"), onPick: (hit) => { q.from = { id: hit.id, label: hit.label }; actions.changed({ ...q }); } });
  const toBox = searchBox({ label: t("to"), placeholder: t("to"), onPick: (hit) => { q.to = { id: hit.id, label: hit.label }; actions.changed({ ...q }); } });
  if (q.from) fromBox.setValue(q.from.label);
  if (q.to) toBox.setValue(q.to.label);
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
      result.replaceChildren(renderPathResult(r, actions));
      // The map always shows this answer (or nothing): never a stale route from an earlier query.
      // An exploration-only route is drawn dashed, like a historical one.
      const m = mapRoute(r);
      actions.showPath(m.steps, m.dashed);
    } catch (err) {
      if ((err as Error).name === "AbortError") return;
      result.replaceChildren(h("p", { class: "notice error", role: "alert" }, `${t("error")}: ${(err as Error).message}`));
    }
  };
  const swap = (): void => {
    [q.from, q.to] = [q.to, q.from];
    fromBox.setValue(q.from?.label ?? "");
    toBox.setValue(q.to?.label ?? "");
    void run();
  };
  const form = h("form", { class: "path-form", onsubmit: (e: Event) => { e.preventDefault(); void run(); } },
    h("div", { class: "ends" },
      h("div", { class: "ends-col" },
        h("div", { class: "row" }, h("span", { class: "lbl" }, t("from")), fromBox.root),
        h("div", { class: "row" }, h("span", { class: "lbl" }, t("to")), toBox.root)),
      h("button", { type: "button", class: "swap", onclick: swap, title: t("swap"), "aria-label": t("swap") }, "⇅")),
    h("div", { class: "row" }, h("label", { class: "lbl", for: "path-objective" }, t("objective")), objective),
    h("details", { open: q.includeDisputed || !!q.weights || q.maxHops !== 6 },
      h("summary", {}, t("advanced")),
      h("div", { class: "row wrap" },
        h("label", { for: "path-hops" }, t("maxHops")), hops,
        h("label", {}, disputed, ` ${t("includeDisputed")}`)),
      h("div", { class: "row" }, h("label", { for: "path-weights" }, t("weights")), weights)),
    h("div", { class: "submit" }, h("button", { type: "submit", class: "primary" }, t("findPath"))),
  );
  if (q.from && q.to) void run();
  return h("section", { class: "pathview", "aria-label": t("path") }, h("h2", {}, t("path")), form, result);
}
