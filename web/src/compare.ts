// Compare two regions (or a region and an entity) side by side, with the
// aggregated edges between them that the loaded tiles carry.
import { h } from "./dom.ts";
import { badge } from "./evidence.ts";
import { relLabel, t, typeLabel } from "./i18n.ts";
import type { RegionInfo } from "./map/map.ts";
import { relationName, stateName, typeName } from "./vocab.ts";

function column(r: RegionInfo, open: (id: string) => void, focus: (r: RegionInfo) => void): HTMLElement {
  const max = Math.max(1, ...r.relations.map(([, n]) => n));
  const isRegion = r.item.kind === "cluster";
  return h("div", { class: "compare-col" },
    h("h3", {}, r.item.label),
    h("p", { class: "muted" }, isRegion ? `${t("region")} ${r.item.id}` : `${typeLabel(typeName(r.item.type))} · ${r.item.id}`),
    h("dl", {},
      h("dt", {}, t("members")), h("dd", {}, String(r.members)),
      h("dt", {}, t("hiddenMembers")), h("dd", {}, String(r.hidden))),
    h("h4", {}, t("internalRelations")),
    r.relations.length
      ? h("ul", { class: "bars" }, r.relations.map(([rel, n]) =>
        h("li", {}, h("span", { class: "bar-label" }, relLabel(relationName(rel))),
          h("span", { class: "bar", style: `width:${Math.max(2, (100 * n) / max)}%`, "aria-hidden": "true" }),
          h("span", { class: "bar-n" }, String(n)))))
      : h("p", { class: "muted" }, t("none")),
    r.central.length ? h("h4", {}, t("central")) : null,
    r.central.length ? h("ul", {}, r.central.map((id) => h("li", {}, h("button", { class: "link", type: "button", onclick: () => open(id) }, id)))) : null,
    isRegion ? h("button", { type: "button", onclick: () => focus(r) }, t("focusRegion")) : null,
  );
}

export function renderCompare(a: RegionInfo | null, b: RegionInfo | null, between: Map<string, number>, open: (id: string) => void, focus: (r: RegionInfo) => void): HTMLElement {
  const root = h("section", { class: "compare", "aria-label": t("compare") }, h("h2", {}, t("compare")));
  if (!a || !b) {
    root.append(h("p", { class: "muted" }, t("compareHint")));
    if (a) root.append(column(a, open, focus));
    return root;
  }
  const rows = [...between].sort((x, y) => y[1] - x[1]);
  root.append(
    h("div", { class: "compare-grid" }, column(a, open, focus), column(b, open, focus)),
    h("h3", {}, t("between")),
    rows.length
      ? h("ul", {}, rows.map(([key, n]) => {
        const [rel, st] = key.split(":").map(Number);
        return h("li", {}, `${relLabel(relationName(rel))} × ${n} `, badge(stateName(st)));
      }))
      : h("p", { class: "muted" }, t("none")),
  );
  return root;
}
