// Path view: typed routes with per-step justification and what the whole route
// authorizes (strength 0..5, weakest evidence, hypothesis, missing premises).
import { api, type FoundPath, type PathResponse, type PathStep } from "./api.ts";
import { h } from "./dom.ts";
import { badge } from "./evidence.ts";
import { getLang, OBJECTIVE_NAMES, objectiveLabel, relLabel, t } from "./i18n.ts";
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
  showPath(steps: PathStep[]): void;
  changed(q: PathQueryState): void;
}

const MAX_STRENGTH = 5;

function strengthMeter(n: number): HTMLElement {
  const m = h("meter", { min: 0, max: MAX_STRENGTH, value: n, "aria-label": `${t("strength")} ${n}/${MAX_STRENGTH}` });
  return h("span", { class: "strength" }, m, ` ${n}/${MAX_STRENGTH}`);
}

function stepItem(s: PathStep, i: number, actions: PathViewActions): HTMLElement {
  const link = (id: string, label: string) => h("button", { class: "link", type: "button", onclick: () => actions.open(id) }, label || id);
  const premises = s.premises.length
    ? h("div", { class: "premises" }, `${t("premises")}: `,
      ...s.premises.map((p) => {
        const missing = s.premises_missing.includes(p);
        const how = s.premises_satisfied.find((x) => x.premise === p);
        return h("span", { class: missing ? "premise missing" : "premise ok", title: how ? `${how.how}: ${how.claims.join(", ")}` : "" },
          missing ? "✗ " : "✓ ", link(p, p), how ? ` (${how.how})` : "", " ");
      }))
    : null;
  return h("li", { class: "step" },
    h("div", { class: "step-head" },
      h("span", { class: "step-n" }, `${t("step")} ${i + 1}`), " ",
      link(s.from, s.from_label), h("span", { class: "rel" }, ` — ${relLabel(s.relation)} → `), link(s.to, s.to_label),
      s.direction === "inverse" ? h("span", { class: "tag" }, `↺ ${t("inverse")}`) : null, " ", badge(s.evidence_state)),
    h("div", {}, h("strong", {}, `${t("authorizes")}: `), s.authorizes),
    h("div", { class: "muted" }, `${t("routeSoFar")}: ${s.route_so_far}`),
    s.assumptions.length ? h("div", { class: "assumptions" }, `${t("assumptions")}: ${s.assumptions.join("; ")}`) : null,
    premises,
    s.via.length ? h("div", { class: "muted" }, `${t("via")}: `, ...s.via.map((v) => link(v, v))) : null,
    s.validity_domain ? h("div", { class: "muted" }, `⟂ ${s.validity_domain}`) : null,
    s.alternatives.length ? h("div", { class: "muted" }, `${t("alternatives")}: ${s.alternatives.join(", ")}`) : null,
    h("div", { class: "prov" }, `${s.claim_id} · `, s.sources.map((src) => `${src.source_id} · ${src.locator} (${src.license})`).join("; ")),
  );
}

function pathBlock(p: FoundPath, title: string, actions: PathViewActions): HTMLElement {
  const c = p.conclusion;
  return h("section", { class: "path" },
    h("h3", {}, title),
    h("div", { class: "conclusion" },
      h("div", {}, h("strong", {}, `${t("conclusion")}: `), c.authorizes, h("span", { class: "muted" }, ` (${c.kind})`)),
      h("div", {}, `${t("strength")}: `, strengthMeter(c.strength), " · ", badge(c.weakest_evidence),
        c.is_hypothesis ? h("span", { class: "tag warn" }, t("hypothesis")) : null,
        c.complete ? null : h("span", { class: "tag warn" }, t("incomplete"))),
      c.missing_premises.length ? h("div", {}, `${t("missingPremises")}: ${c.missing_premises.join(", ")}`) : null,
      c.assumptions.length ? h("div", { class: "assumptions" }, `${t("assumptions")}: ${c.assumptions.join("; ")}`) : null,
      c.caveats.length ? h("ul", { class: "caveats", "aria-label": t("caveats") }, c.caveats.map((x) => h("li", {}, x))) : null,
      h("div", { class: "muted small" }, `score ${p.score.toFixed(3)} · `, Object.entries(p.cost_terms).map(([k, v]) => `${k} ${v.toFixed(2)}`).join(", ")),
    ),
    h("button", { type: "button", onclick: () => actions.showPath(p.steps) }, t("showOnMap")),
    h("ol", { class: "steps" }, p.steps.map((s, i) => stepItem(s, i, actions))),
  );
}

export function renderPathResult(r: PathResponse, actions: PathViewActions): HTMLElement {
  const root = h("div", { class: "path-result", "aria-live": "polite" });
  root.append(h("p", { class: "muted small" }, `${t("weights")}: `, Object.entries(r.weights).map(([k, v]) => `${k}:${v}`).join(", "),
    ` · ${r.stats.algorithm} · ${r.stats.expanded} exp · ${(r.stats.micros / 1000).toFixed(1)} ms`));
  if (r.abstained) {
    root.append(h("div", { class: "notice", role: "note" }, h("strong", {}, `${t("abstained")}: `), r.reason ?? r.abstention ?? ""));
    if (r.exploration) root.append(pathBlock(r.exploration, t("exploration"), actions));
    return root;
  }
  r.paths.forEach((p, i) => root.append(pathBlock(p, `${t("path")} ${i + 1}`, actions)));
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
      const r = await api.path({ from: q.from.id, to: q.to.id, objective: q.objective, max_hops: q.maxHops, include_disputed: q.includeDisputed, lang: getLang(), weights: q.weights || undefined }, ctrl.signal);
      result.replaceChildren(renderPathResult(r, actions));
      if (!r.abstained && r.paths[0]) actions.showPath(r.paths[0].steps);
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
    h("div", { class: "row" }, h("span", { class: "lbl" }, t("from")), fromBox.root),
    h("div", { class: "row" }, h("span", { class: "lbl" }, t("to")), toBox.root),
    h("div", { class: "row wrap" },
      h("label", { for: "path-objective" }, t("objective")), objective,
      h("label", { for: "path-hops" }, t("maxHops")), hops,
      h("label", {}, disputed, ` ${t("includeDisputed")}`)),
    h("div", { class: "row" }, h("label", { for: "path-weights" }, t("weights")), weights),
    h("div", { class: "row" }, h("button", { type: "submit", class: "primary" }, t("findPath")), h("button", { type: "button", onclick: swap }, `⇅ ${t("swap")}`)),
  );
  if (q.from && q.to) void run();
  return h("section", { class: "pathview", "aria-label": t("path") }, h("h2", {}, t("path")), form, result);
}
