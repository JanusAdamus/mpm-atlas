// Instant search as an ARIA combobox: results while typing (debounced, previous
// request aborted), arrow keys to move, Enter to pick, Escape to close.
import { api, type SearchHit } from "./api.ts";
import { h } from "./dom.ts";
import { getLang, isExplore, t, typeLabel } from "./i18n.ts";
import { isCodeId, jargonIn } from "./vocab.ts";

const DEBOUNCE_MS = 90;

/** Reader-facing name of the field a hit matched (the API sends `description`, `alias`, …). */
export const fieldLabel = (field: string): string => {
  const s = t(`field_${field}`);
  return s === `field_${field}` ? field.replaceAll("_", " ") : s;
};
/** The explore mode leaves code records (and labels that are ids) to the technical one. */
export function readerHits(hits: SearchHit[]): { hits: SearchHit[]; hidden: number } {
  if (!isExplore()) return { hits, hidden: 0 };
  const kept = hits.filter((x) => !isCodeId(x.id) && !jargonIn(x.label).length);
  return { hits: kept, hidden: hits.length - kept.length };
}

/** Matched text worth showing next to a hit: never an external id in the explore mode. */
const showMatch = (hit: SearchHit): boolean => hit.matched.field !== "label" && !(isExplore() && (hit.matched.field === "external_id" || jargonIn(hit.matched.text).length > 0));

let uid = 0;

export interface SearchBox {
  root: HTMLElement;
  input: HTMLInputElement;
  setValue(label: string): void;
  /** After a language change: new label and placeholder; the last announcement is cleared,
   *  so no «5 results» stays in a Spanish page (C4-09). */
  relabel(label: string, placeholder: string): void;
}

export function searchBox(opts: { label: string; placeholder: string; onPick(hit: SearchHit): void; limit?: number }): SearchBox {
  const id = `sb${++uid}`;
  const input = h("input", {
    type: "search", id, role: "combobox", "aria-autocomplete": "list", "aria-expanded": "false",
    "aria-controls": `${id}-list`, placeholder: opts.placeholder, autocomplete: "off", spellcheck: "false",
  });
  const list = h("ul", { id: `${id}-list`, role: "listbox", class: "results", hidden: true, "aria-label": opts.label });
  const status = h("span", { class: "sr-only", role: "status", "aria-live": "polite" });
  const labelEl = h("label", { for: id, class: "sr-only" }, opts.label);
  const root = h("div", { class: "searchbox" }, labelEl, input, list, status);
  let hits: SearchHit[] = [];
  let hiddenTech = 0;
  let active = -1;
  let ctrl: AbortController | null = null;
  let timer = 0;

  const close = (): void => {
    list.hidden = true;
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
    active = -1;
  };
  const highlight = (i: number): void => {
    active = i;
    [...list.children].forEach((li, k) => li.setAttribute("aria-selected", String(k === i)));
    if (i >= 0) {
      input.setAttribute("aria-activedescendant", `${id}-o${i}`);
      list.children[i]?.scrollIntoView({ block: "nearest" });
    }
  };
  const pick = (i: number): void => {
    const hit = hits[i];
    if (!hit) return;
    close();
    input.value = hit.label;
    opts.onPick(hit);
  };
  const render = (): void => {
    list.replaceChildren(
      ...hits.map((hit, i) =>
        h("li", { id: `${id}-o${i}`, role: "option", "aria-selected": "false", onmousedown: (e: Event) => { e.preventDefault(); pick(i); } },
          h("span", { class: "hit-label" }, hit.label),
          h("span", { class: "hit-type" }, typeLabel(hit.type)),
          showMatch(hit)
            ? h("span", { class: "hit-match" }, h("span", { class: "field" }, fieldLabel(hit.matched.field)),
              h("span", { class: hit.matched.field === "formula" ? "formula-src" : "" }, hit.matched.text.slice(0, 90)))
            : null,
        ),
      ),
    );
    if (!hits.length) list.append(h("li", { class: "empty", role: "option", "aria-disabled": "true" }, t("noResultsFor", { q: input.value.trim() })));
    if (hiddenTech) list.append(h("li", { class: "empty note", role: "option", "aria-disabled": "true" }, t("searchTechHidden")));
    list.hidden = false;
    input.setAttribute("aria-expanded", "true");
    status.textContent = t("results", { n: hits.length });
    highlight(hits.length ? 0 : -1);
  };
  const run = async (q: string): Promise<void> => {
    ctrl?.abort();
    if (!q.trim()) {
      hits = [];
      close();
      return;
    }
    ctrl = new AbortController();
    try {
      const limit = opts.limit ?? 12;
      // Explore asks for more and drops code records, so a reader still gets a full list.
      const r = await api.search(q, getLang(), isExplore() ? limit * 2 : limit, ctrl.signal);
      const kept = readerHits(r.hits);
      hits = kept.hits.slice(0, limit);
      hiddenTech = kept.hidden;
      render();
    } catch (err) {
      if ((err as Error).name === "AbortError") return;
      hits = [];
      list.replaceChildren(h("li", { class: "empty", role: "option", "aria-disabled": "true" }, `${t("error")}: ${(err as Error).message}`));
      list.hidden = false;
    }
  };
  input.addEventListener("input", () => {
    clearTimeout(timer);
    timer = window.setTimeout(() => void run(input.value), DEBOUNCE_MS);
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "ArrowDown") { e.preventDefault(); if (list.hidden) void run(input.value); else highlight(Math.min(hits.length - 1, active + 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); highlight(Math.max(0, active - 1)); }
    else if (e.key === "Enter") { e.preventDefault(); pick(active < 0 ? 0 : active); }
    else if (e.key === "Escape") { if (!list.hidden) { e.stopPropagation(); close(); } }
  });
  input.addEventListener("blur", () => setTimeout(close, 120));
  const relabel = (label: string, placeholder: string): void => {
    if (labelEl.textContent === label) return; // same language: keep what is open
    labelEl.textContent = label;
    list.setAttribute("aria-label", label);
    input.setAttribute("placeholder", placeholder);
    status.textContent = "";
    close();
  };
  return { root, input, setValue: (label: string) => { input.value = label; }, relabel };
}
