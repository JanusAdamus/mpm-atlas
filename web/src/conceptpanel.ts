// Concept panel: what a concept groups (its declarations, paged, each with how it was
// assigned and from which source) and how it connects to other concepts (aggregated
// edges with counts per relation and evidence state, and a drill-down to the
// declaration-level edges behind each one).  Assignments are candidates unless curated:
// the heuristic score is labelled as such and never shown as a probability.
import type { ConceptEdgeJson, ConceptEdgesResponse, ConceptMember, ConceptResponse } from "./api.ts";
import { h } from "./dom.ts";
import { badge } from "./evidence.ts";
import { isExplore, relLabel, stateLabel, t, typeLabel } from "./i18n.ts";
import { typeColor } from "./theme.ts";

export interface ConceptActions {
  open(id: string): void;
  openConcept(id: string): void;
  /** Unfold on the map: zoom until the concept's declarations are drawn around it. */
  expand(id: string, members: number): void;
  collapse(id: string): void;
  page(offset: number): void;
  drill(from: string, to: string, offset: number): Promise<ConceptEdgesResponse>;
  /** Switch to the technical mode (this list is code). */
  technical?(): void;
}

/** Signals whose assignment is a fallback (zone of the module), shown as low confidence. */
export const FALLBACK_SIGNALS = new Set(["zone", "zone_derived"]);

/** Reader-facing label of how an assignment was made. */
export function signalLabel(signal: string | null | undefined): string {
  return signal ? t(`signal_${signal}`) : t("signal_unknown");
}

/** Plain-language status of one assignment: established (extracted) or a candidate. */
export function assignmentStatus(m: Pick<ConceptMember, "evidence_state" | "signal">): { text: string; low: boolean } {
  const low = FALLBACK_SIGNALS.has(m.signal ?? "");
  if (low) return { text: t("lowConfidence"), low };
  return { text: m.evidence_state === "model_proposed" ? t("candidate") : stateLabel(m.evidence_state), low };
}

function memberRow(m: ConceptMember, actions: ConceptActions): HTMLElement {
  const status = assignmentStatus(m);
  const prov = m.provenance.map((p) =>
    [p.source_id, p.extractor && `${p.extractor}${p.extractor_version ? ` v${p.extractor_version}` : ""}`, p.locator].filter(Boolean).join(" · "));
  const evidence = typeof m.evidence === "string" ? m.evidence : m.evidence ? JSON.stringify(m.evidence) : null;
  return h("li", { class: `claim state-${m.evidence_state}`, "data-claim": m.claim_id },
    h("div", { class: "claim-line" },
      h("button", { class: "link", type: "button", title: m.id, onclick: () => actions.open(m.id) }, m.label || m.id),
      h("span", { class: "muted small" }, ` · ${typeLabel(m.type)}`)),
    h("div", { class: "claim-meta" },
      badge(m.evidence_state),
      h("span", { class: "tag" }, m.primary ? t("primary") : t("secondary")),
      h("span", { class: status.low ? "tag warn" : "tag" }, `${signalLabel(m.signal)}${status.low ? ` · ${status.text}` : ""}`)),
    h("details", { class: "member-details" },
      h("summary", { class: "small" }, t("provenance")),
      evidence ? h("div", { class: "prov wrap" }, `${t("evidenceText")}: ${evidence}`) : null,
      ...prov.map((p) => h("div", { class: "prov wrap" }, p)),
      m.score ? h("div", { class: "muted small", title: `${m.score.estimates} — ${m.score.calibration}` }, `${t("heuristicScore")}: ${m.score.value.toFixed(2)}`) : null,
      h("div", { class: "prov wrap" }, m.claim_id)));
}

function pager(r: ConceptResponse, actions: ConceptActions): HTMLElement | null {
  if (r.memberships_total <= r.limit && r.offset === 0) return null;
  const from = r.members.length ? r.offset + 1 : 0, to = r.offset + r.members.length;
  return h("nav", { class: "pager", "aria-label": t("members") },
    h("button", { type: "button", disabled: r.offset === 0, onclick: () => actions.page(Math.max(0, r.offset - r.limit)) }, `← ${t("prev")}`),
    h("span", { class: "muted small" }, t("pageInfo", { a: from, b: to, n: r.memberships_total.toLocaleString() })),
    h("button", { type: "button", disabled: !r.truncated, onclick: () => actions.page(r.offset + r.limit) }, `${t("next")} →`));
}

function drillList(res: ConceptEdgesResponse, actions: ConceptActions, more: () => void): HTMLElement {
  const shown = res.offset + res.edges.length;
  return h("div", { class: "drill" },
    h("ul", { class: "claims" }, res.edges.map((e) => h("li", { class: `claim state-${e.evidence_state}`, "data-claim": e.claim_id },
      h("div", { class: "claim-line" },
        h("button", { class: "link", type: "button", title: e.source.id, onclick: () => actions.open(e.source.id) }, e.source.label),
        h("span", { class: "dir" }, ` ${relLabel(e.relation)} `),
        h("button", { class: "link", type: "button", title: e.target.id, onclick: () => actions.open(e.target.id) }, e.target.label)),
      h("div", { class: "claim-meta" }, badge(e.evidence_state))))),
    h("p", { class: "muted small" }, t("drillCount", { n: shown.toLocaleString(), m: res.total.toLocaleString() }), res.complete ? "" : ` ${t("drillPartial")}`),
    shown < res.total ? h("button", { type: "button", class: "mini", onclick: more }, t("next")) : null);
}

/** One aggregated edge as a sentence per (relation, state): "112 declarations … depend logically on …". */
function edgeRow(self: ConceptResponse, e: ConceptEdgeJson, out: boolean, actions: ConceptActions): HTMLElement {
  const other = h("button", { class: "link", type: "button", title: e.concept.id, onclick: () => actions.openConcept(e.concept.id) },
    e.concept.label + (e.concept.derived ? ` (${t("derivedShort")})` : ""));
  const [from, to] = out ? [self.concept.id, e.concept.id] : [e.concept.id, self.concept.id];
  const host = h("div");
  let offset = 0;
  const load = (): void => {
    void actions.drill(from, to, offset).then((res) => {
      offset = res.offset + res.edges.length;
      host.replaceChildren(drillList(res, actions, load));
    }).catch((err: Error) => host.replaceChildren(h("p", { class: "notice error", role: "alert" }, `${t("error")}: ${err.message}`)));
  };
  return h("li", { class: "concept-edge" },
    h("div", { class: "claim-line" }, out ? h("span", { class: "dir" }, "→ ") : h("span", { class: "dir" }, "← "), other,
      h("span", { class: "muted small" }, ` · ${e.total.toLocaleString()}`),
      e.candidate ? h("span", { class: "tag candidate", title: t("conceptCandidate") }, ` ${t("candidate")}`) : null),
    h("ul", { class: "edge-stats" }, e.stats.map((s) => h("li", {},
      t(out ? "edgeOut" : "edgeIn", { n: s.count.toLocaleString(), rel: relLabel(s.relation) }), " ", badge(s.evidence_state)))),
    h("details", { ontoggle: (ev: Event) => { if ((ev.target as HTMLDetailsElement).open && !host.childElementCount) load(); } },
      h("summary", { class: "small" }, t("showDecls")), host));
}

function edgeSection(r: ConceptResponse, out: boolean, actions: ConceptActions): HTMLElement {
  const list = out ? r.edges_out : r.edges_in;
  const total = out ? r.edges_out_total : r.edges_in_total;
  return h("section", {},
    h("h3", {}, `${t(out ? "edgesOut" : "edgesIn")} (${total.toLocaleString()})`),
    list.length
      ? h("ul", { class: "concept-edges" }, list.map((e) => edgeRow(r, e, out, actions)))
      : h("p", { class: "muted" }, t("noConceptEdges")),
    total > list.length ? h("p", { class: "muted small" }, t("moreEdges", { n: list.length, m: total.toLocaleString() })) : null);
}

/** In the explore mode a concept's list is code: say so and offer the way in, nothing else. */
function exploreConcept(r: ConceptResponse, actions: ConceptActions): HTMLElement {
  const c = r.concept;
  return h("article", { class: "entity concept", "aria-labelledby": "entity-title" },
    h("header", {}, h("p", { class: "eyebrow" }, t("concept")), h("h2", { id: "entity-title", tabindex: -1 }, c.label)),
    h("p", { class: "notice concept-note", role: "note" }, t("techModeNote")),
    h("div", { class: "actions" },
      actions.technical ? h("button", { type: "button", class: "primary", onclick: () => actions.technical?.() }, t("toTechnical")) : null,
      c.derived ? null : h("button", { type: "button", onclick: () => actions.open(c.id) }, t("fullRecord", { name: c.label }))));
}

export function renderConcept(r: ConceptResponse, actions: ConceptActions): HTMLElement {
  if (isExplore()) return exploreConcept(r, actions);
  const c = r.concept;
  const dark = document.documentElement.dataset.theme === "dark";
  const states = Object.entries(r.memberships_by_state).map(([s, n]) => h("span", {}, badge(s), ` ${n.toLocaleString()} `));
  return h("article", { class: "entity concept", "aria-labelledby": "entity-title" },
    h("header", {},
      h("p", { class: "eyebrow" },
        h("span", { class: "concept-mark", style: `color:${typeColor(c.type, dark)}`, "aria-hidden": "true" }),
        c.derived ? t("derivedConcept") : t("concept")),
      h("h2", { id: "entity-title", tabindex: -1 }, c.label),
      h("p", { class: "id" }, c.id)),
    c.derived ? h("p", { class: "notice", role: "note" }, t("derivedNote")) : null,
    h("p", { class: "notice concept-note", role: "note" }, t("conceptListNote")),
    r.description ? h("p", { class: "lede" }, r.description) : null,
    h("div", { class: "actions" },
      r.members_total > 0 ? h("button", { type: "button", onclick: () => actions.expand(c.id, r.members_total) }, t("expand")) : null,
      h("button", { type: "button", onclick: () => actions.collapse(c.id) }, t("collapse")),
      h("button", { type: "button", onclick: () => actions.open(c.id) }, t("fullRecord", { name: c.label }))),
    h("section", {},
      h("h3", {}, t("conceptDecls")),
      h("p", {}, t("conceptMembers", { n: r.members_total.toLocaleString(), m: r.memberships_total.toLocaleString() })),
      h("p", { class: "claim-meta" }, ...states),
      h("p", { class: "muted small" }, t("assignmentNote")),
      pager(r, actions),
      r.members.length ? h("ul", { class: "claims" }, r.members.map((m) => memberRow(m, actions))) : h("p", { class: "muted" }, t("none")),
      pager(r, actions)),
    edgeSection(r, true, actions),
    edgeSection(r, false, actions),
    h("p", { class: "muted small" }, t("edgesNote")));
}
