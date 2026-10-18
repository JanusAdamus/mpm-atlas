// Equivalent text view of the map: what is in view (regions, then entities in the
// same priority as the labels), what is counted but not drawn, and the legend.
// Every map action has a button here, so the map is never the only way in.
import { h } from "./dom.ts";
import { STATE_STYLE, swatch } from "./evidence.ts";
import { relLabel, stateLabel, t, typeLabel } from "./i18n.ts";
import type { MapItem } from "./map/map.ts";
import { CURRENT_STATES, relationName, typeName } from "./vocab.ts";

export interface TextActions {
  open(item: MapItem): void;
  openConcept(item: MapItem): void;
  zoomInto(item: MapItem): void;
  compare(item: MapItem): void;
}

export function legend(): HTMLElement {
  return h("section", { class: "legend", "aria-label": t("legend") },
    h("h3", {}, t("evidence")),
    h("ul", {}, CURRENT_STATES.map((s) => h("li", {}, swatch(s), ` ${STATE_STYLE[s].glyph} ${stateLabel(s)}`))),
    h("h3", {}, t("legendShapes")),
    h("ul", { class: "small" },
      h("li", {}, t("legendConcept")),
      h("li", {}, t("legendRegion")),
      h("li", {}, t("legendEntity"))),
    h("p", { class: "muted small" }, t("legendZoom")),
    h("p", { class: "muted small" }, t("legendLines")),
    h("p", { class: "muted small" }, t("legendProximity")),
  );
}

export function renderTextView(items: MapItem[], hidden: Map<number, number>, actions: TextActions, drawnEdges = -1): HTMLElement {
  const regions = items.filter((i) => i.kind === "cluster");
  const concepts = items.filter((i) => i.kind === "concept");
  const entities = items.filter((i) => i.kind === "entity");
  const hiddenRows = [...hidden].sort((a, b) => b[1] - a[1]);
  return h("div", { class: "textview" },
    h("section", { "aria-label": t("visibleRegions") },
      h("h3", {}, `${t("visibleRegions")} (${regions.length})`),
      h("ul", {}, regions.map((r) => h("li", {},
        h("button", { type: "button", class: "link", onclick: () => actions.zoomInto(r) }, r.label),
        h("span", { class: "muted" }, ` · ${t("members")}: ${r.size} `),
        h("button", { type: "button", class: "mini", onclick: () => actions.compare(r) }, t("compareWith")))))),
    concepts.length
      ? h("section", { "aria-label": t("visibleConcepts") },
        h("h3", {}, `${t("visibleConcepts")} (${concepts.length})`),
        h("ul", {}, concepts.map((c) => h("li", {},
          h("button", { type: "button", class: "link", onclick: () => actions.openConcept(c) }, c.label),
          h("span", { class: "muted" }, ` · ${t("members")}: ${c.size.toLocaleString()} `),
          // Nothing to unfold when no declaration has it as primary concept.
          c.size > 0 ? h("button", { type: "button", class: "mini", onclick: () => actions.zoomInto(c) }, t("expand")) : null))))
      : null,
    h("section", { "aria-label": t("visibleEntities") },
      h("h3", {}, `${t("visibleEntities")} (${entities.length})`),
      h("ul", {}, entities.map((e) => h("li", {},
        h("button", { type: "button", class: "link", onclick: () => actions.open(e) }, e.label),
        h("span", { class: "muted" }, ` · ${typeLabel(typeName(e.type))}`))))),
    hiddenRows.length
      ? h("section", {}, h("h3", {}, t("hiddenEdges")),
        h("ul", {}, hiddenRows.map(([rel, n]) => h("li", {}, `${relLabel(relationName(rel))}: ${n.toLocaleString()}`))))
      : null,
    drawnEdges === 0 ? h("p", { class: "muted small" }, t("noEdgesInView")) : null,
    legend(),
  );
}
