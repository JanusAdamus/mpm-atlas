// Explanation panel of one entity: intuition, formulation (KaTeX), assumptions,
// connections grouped by relation with evidence badges, sources with license.
import type { ClaimJson, EntityResponse } from "./api.ts";
import { h, safeLink } from "./dom.ts";
import { badge } from "./evidence.ts";
import { getLang, relLabel, stateLabel, t, typeLabel } from "./i18n.ts";

export interface PanelActions {
  open(id: string): void;
  showOnMap(id: string): void;
  focusRegion(id: string): void;
  pathFrom(id: string, label: string): void;
  pathTo(id: string, label: string): void;
}

/** Text in the UI language, else the other one, tagged with its language. */
function localized(m: Record<string, string>): { text: string; lang: string } | null {
  const l = getLang();
  if (m[l]) return { text: m[l], lang: l };
  const other = Object.entries(m)[0];
  return other ? { text: other[1], lang: other[0] } : null;
}

let math: Promise<typeof import("./math.ts")> | null = null;

/** Typeset `tex` into `el` once KaTeX has loaded; until then (or on error) the source shows. */
function typeset(el: HTMLElement, tex: string, display: boolean): void {
  el.textContent = tex;
  math ??= import("./math.ts");
  math.then((m) => m.renderMath(el, tex, display)).catch(() => {
    el.textContent = tex;
  });
}

export function renderFormula(tex: string): HTMLElement {
  const el = h("div", { class: "formula", role: "math", "aria-label": tex });
  typeset(el, tex, true);
  return el;
}

/** Inline math in prose: `$…$` segments typeset with KaTeX, the rest as text. */
export function richText(s: string): HTMLElement {
  const el = h("span");
  for (const p of s.split(/(\$[^$\n]{1,400}\$)/g)) {
    if (p.length > 2 && p.startsWith("$") && p.endsWith("$")) {
      const span = h("span", { class: "math-inline" });
      typeset(span, p.slice(1, -1), false);
      el.append(span);
    } else if (p) el.append(p);
  }
  return el;
}

function entityLink(id: string, label: string | undefined, actions: PanelActions): HTMLElement {
  return h("button", { class: "link", type: "button", title: id, onclick: () => actions.open(id) }, label || id);
}

function claimRow(c: ClaimJson, self: string, selfIds: Set<string>, actions: PanelActions): HTMLElement {
  const role = c.participants.find((p) => selfIds.has(p.entity))?.role ?? "subject";
  const others = (roles: string[]) => c.participants.filter((p) => roles.includes(p.role) && !selfIds.has(p.entity));
  const targets = others(["object", "premise"]);
  const premises = c.participants.filter((p) => p.role === "premise");
  const subject = c.participants.find((p) => p.role === "subject");
  const label = (id: string) => c.labels[id];
  const line = h("div", { class: "claim-line" });
  if (role === "subject") {
    line.append(h("span", { class: "dir", "aria-label": "→" }, "→ "));
    targets.forEach((p, i) => {
      if (i > 0) line.append(h("span", { class: "and" }, premises.length > 1 ? " + " : ", "));
      line.append(entityLink(p.entity, label(p.entity), actions));
    });
  } else if (subject) {
    line.append(h("span", { class: "dir", "aria-label": "←" }, "← "), entityLink(subject.entity, label(subject.entity), actions));
    if (role === "premise" && premises.length > 1) {
      line.append(h("span", { class: "muted" }, ` (${t("premises").toLowerCase()}: ${premises.length})`));
    } else if (role !== "object") line.append(h("span", { class: "muted" }, ` (${role})`));
  }
  const meta = h("div", { class: "claim-meta" }, badge(c.evidence_state));
  if (premises.length > 1) meta.append(h("span", { class: "tag" }, `${t("premises")} (Y/AND): ${premises.length}`));
  if (c.alternative_group) meta.append(h("span", { class: "tag", title: c.alternative_group }, `${t("alternatives")}: ${c.alternative_group}`));
  if (c.formal) meta.append(h("span", { class: "tag" }, `⊢ ${c.formal.library} ${c.formal.version}: ${c.formal.declaration}`));
  if (c.confidence) meta.append(h("span", { class: "tag", title: `${c.confidence.estimates} · ${c.confidence.calibration}` }, `p=${c.confidence.value.toFixed(2)}`));
  const extra: HTMLElement[] = [];
  const via = others(["via"]), ctx = others(["context"]), assm = others(["assumption"]);
  if (via.length) extra.push(h("div", { class: "muted" }, `${t("via")}: `, ...via.map((p) => entityLink(p.entity, label(p.entity), actions))));
  if (ctx.length) extra.push(h("div", { class: "muted" }, "context: ", ...ctx.map((p) => entityLink(p.entity, label(p.entity), actions))));
  if (c.assumptions.length || assm.length) {
    extra.push(h("div", { class: "assumptions" }, `${t("assumptions")}: `, c.assumptions.join("; "), ...assm.map((p) => entityLink(p.entity, label(p.entity), actions))));
  }
  if (c.validity_domain) extra.push(h("div", { class: "muted" }, `⟂ ${c.validity_domain}`));
  const expl = localized(c.explanation);
  if (expl) extra.push(h("div", { class: "explanation", lang: expl.lang }, richText(expl.text)));
  if (c.disagreements.length) extra.push(h("div", { class: "muted" }, `≠ ${c.disagreements.join("; ")}`));
  const prov = c.provenance.map((p) => `${p.source_id} · ${p.locator}`).join("; ");
  return h("li", { class: `claim state-${c.evidence_state}`, "data-claim": c.id, title: `${c.id} — ${self}` }, line, meta, ...extra, h("div", { class: "prov" }, prov));
}

export function renderEntity(r: EntityResponse, actions: PanelActions): HTMLElement {
  const e = r.entity;
  const selfIds = new Set([r.canonical_id, ...r.merged_from]);
  const root = h("article", { class: "entity", "aria-labelledby": "entity-title" });
  root.append(
    h("header", {},
      h("p", { class: "eyebrow" }, typeLabel(e.type)),
      h("h2", { id: "entity-title", tabindex: -1 }, r.label),
      h("p", { class: "id" }, e.id),
      r.merged_from.length ? h("p", { class: "muted" }, `${t("mergedFrom")}: ${r.merged_from.join(", ")}`) : null,
    ),
    h("div", { class: "actions" },
      h("button", { type: "button", onclick: () => actions.showOnMap(r.canonical_id) }, t("showOnMap")),
      h("button", { type: "button", onclick: () => actions.focusRegion(r.canonical_id) }, t("focusRegion")),
      h("button", { type: "button", onclick: () => actions.pathFrom(r.canonical_id, r.label) }, t("pathFrom")),
      h("button", { type: "button", onclick: () => actions.pathTo(r.canonical_id, r.label) }, t("pathTo")),
    ),
  );
  if (r.coverage === "unknown") root.append(h("p", { class: "notice", role: "note" }, t("coverageUnknown")));
  const intu = localized(e.descriptions);
  if (intu) root.append(h("section", {}, h("h3", {}, t("intuition")), h("p", { lang: intu.lang }, richText(intu.text))));
  const form = localized(e.statements);
  if (form || e.formula) {
    root.append(h("section", {}, h("h3", {}, t("formulation")), form ? h("p", { lang: form.lang }, richText(form.text)) : null, e.formula ? renderFormula(e.formula) : null));
  }
  if (e.aliases.length) root.append(h("p", { class: "muted" }, `${t("aliases")}: `, e.aliases.map(([lang, text]) => `${text} (${lang})`).join(", ")));
  if (e.external_ids.length) root.append(h("p", { class: "muted" }, `${t("externalIds")}: `, e.external_ids.map(([s, v]) => `${s}:${v}`).join(", ")));

  const assumptions = new Set<string>();
  for (const c of r.claims) for (const a of c.assumptions) assumptions.add(a);
  if (assumptions.size) root.append(h("section", {}, h("h3", {}, t("assumptions")), h("ul", {}, [...assumptions].map((a) => h("li", {}, richText(a))))));

  const groups = new Map<string, ClaimJson[]>();
  for (const c of r.claims) groups.set(c.relation, [...(groups.get(c.relation) ?? []), c]);
  if (groups.size) {
    const sec = h("section", {}, h("h3", {}, t("connections")));
    for (const [rel, claims] of [...groups].sort((a, b) => b[1].length - a[1].length)) {
      const states = new Map<string, number>();
      for (const c of claims) states.set(c.evidence_state, (states.get(c.evidence_state) ?? 0) + 1);
      const summary = [...states].map(([s, n]) => `${n} ${stateLabel(s)}`).join(", ");
      sec.append(
        h("details", { open: claims.length <= 12 },
          h("summary", {}, h("strong", {}, relLabel(rel)), ` · ${r.counts_by_relation[rel] ?? claims.length} `, h("span", { class: "muted" }, `(${summary})`)),
          h("ul", { class: "claims" }, claims.map((c) => claimRow(c, r.canonical_id, selfIds, actions))),
        ),
      );
    }
    if (r.truncated) sec.append(h("p", { class: "muted" }, t("claimsTruncated", { n: r.claims.length })));
    root.append(sec);
  }

  const sources = Object.values(r.sources);
  if (sources.length) {
    root.append(h("section", {}, h("h3", {}, t("sources")),
      h("ul", { class: "sources" }, sources.map((s) => h("li", {},
        safeLink(s.uri, s.title || s.id), " ",
        h("span", { class: "license" }, `${t("license")}: ${s.license}`),
        s.version ? h("span", { class: "muted" }, ` · v${s.version}`) : null,
        s.terms ? h("div", { class: "muted small" }, s.terms) : null,
      )))));
  }
  return root;
}
