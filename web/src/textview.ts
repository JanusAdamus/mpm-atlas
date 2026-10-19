// Equivalent text view of the map: what is in view (regions, then entities in the
// same priority as the labels), what is counted but not drawn, and the legend.
// Every map action has a button here, so the map is never the only way in.  The explore
// mode lists ideas only: no code records, ids or internal counts.
import { h } from "./dom.ts";
import { legendList } from "./evidence.ts";
import { isExplore, relLabel, t, typeLabel } from "./i18n.ts";
import type { MapItem } from "./map/map.ts";
import { isCodeId, jargonIn, relationName, typeName } from "./vocab.ts";

export interface TextActions {
  open(item: MapItem): void;
  openConcept(item: MapItem): void;
  zoomInto(item: MapItem): void;
  compare(item: MapItem): void;
}

export function legend(): HTMLElement {
  const explore = isExplore();
  return h("section", { class: "legend", "aria-label": t("legend") },
    h("h3", {}, t("legendTitle")),
    legendList(true),
    h("h3", {}, t("legendShapes")),
    h("p", { class: "small" }, t("legendDotsPlain")),
    h("ul", { class: "small" },
      h("li", {}, t(explore ? "legendConceptPlain" : "legendConcept")),
      h("li", {}, t(explore ? "legendRegionPlain" : "legendRegion")),
      h("li", {}, t(explore ? "legendEntityPlain" : "legendEntity"))),
    h("p", { class: "muted small" }, t(explore ? "legendZoomPlain" : "legendZoom")),
    explore ? null : h("p", { class: "muted small" }, t("legendLines")),
    h("p", { class: "muted small" }, t("legendProximity")),
  );
}

/** Items the explore mode can list: no code records, and no label that is itself jargon (an id, «Mathlib»). */
export const readerItems = (items: MapItem[]): MapItem[] =>
  items.filter((it) => !isCodeId(it.id) && it.label && !jargonIn(it.label).length);

/** `regionName`: the name the map gives a region (map.regionName), so the index never lists
 *  a region as «Wikipedia (es): …»; null = no lettered member loaded, read as «región». */
export function renderTextView(all: MapItem[], hidden: Map<number, number>, actions: TextActions, drawnEdges = -1,
  regionName: (it: MapItem) => string | null = (it) => it.label): HTMLElement {
  const explore = isExplore();
  const items = explore ? readerItems(all) : all;
  const regions = items.filter((i) => i.kind === "cluster");
  const concepts = items.filter((i) => i.kind === "concept");
  const entities = items.filter((i) => i.kind === "entity");
  const hiddenRows = [...hidden].sort((a, b) => b[1] - a[1]);
  const heading = (key: string, n: number): string => (explore ? t(key) : `${t(key)} (${n})`);
  const members = (n: number): string | null => (explore ? null : ` · ${t("members")}: ${n.toLocaleString()} `);
  return h("div", { class: "textview" },
    h("section", { "aria-label": t("visibleRegions") },
      h("h3", {}, heading("visibleRegions", regions.length)),
      h("ul", {}, regions.map((r) => h("li", {},
        h("button", { type: "button", class: "link", onclick: () => actions.zoomInto(r) }, regionName(r) ?? t("region")),
        h("span", { class: "muted" }, members(r.size) ?? " "),
        h("button", { type: "button", class: "mini", onclick: () => actions.compare(r) }, t("compareWith")))))),
    concepts.length
      ? h("section", { "aria-label": t("visibleConcepts") },
        h("h3", {}, heading("visibleConcepts", concepts.length)),
        h("ul", {}, concepts.map((c) => h("li", {},
          h("button", { type: "button", class: "link", onclick: () => actions.openConcept(c) }, c.label),
          explore ? null : h("span", { class: "muted" }, members(c.size)),
          // Nothing to unfold when no declaration has it as primary concept.
          !explore && c.size > 0 ? h("button", { type: "button", class: "mini", onclick: () => actions.zoomInto(c) }, t("expand")) : null))))
      : null,
    h("section", { "aria-label": t(explore ? "visibleIdeas" : "visibleEntities") },
      h("h3", {}, heading(explore ? "visibleIdeas" : "visibleEntities", entities.length)),
      h("ul", {}, entities.map((e) => h("li", {},
        h("button", { type: "button", class: "link", onclick: () => actions.open(e) }, e.label),
        h("span", { class: "muted" }, ` · ${typeLabel(typeName(e.type))}`))))),
    hiddenRows.length && !explore
      ? h("section", {}, h("h3", {}, t("hiddenEdges")),
        h("ul", {}, hiddenRows.map(([rel, n]) => h("li", {}, `${relLabel(relationName(rel))}: ${n.toLocaleString()}`))))
      : null,
    drawnEdges === 0 ? h("p", { class: "muted small" }, t("noEdgesInView")) : null,
    legend(),
  );
}
