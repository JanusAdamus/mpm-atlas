// Instant search as an ARIA combobox: results while typing (debounced, previous
// request aborted), arrow keys to move, Enter to pick, Escape to close.
import { api, type SearchHit } from "./api.ts";
import { h } from "./dom.ts";
import { getLang, t, typeLabel } from "./i18n.ts";

const DEBOUNCE_MS = 90;
let uid = 0;

export interface SearchBox {
  root: HTMLElement;
  input: HTMLInputElement;
  setValue(label: string): void;
}

export function searchBox(opts: { label: string; placeholder: string; onPick(hit: SearchHit): void; limit?: number }): SearchBox {
  const id = `sb${++uid}`;
  const input = h("input", {
    type: "search", id, role: "combobox", "aria-autocomplete": "list", "aria-expanded": "false",
    "aria-controls": `${id}-list`, placeholder: opts.placeholder, autocomplete: "off", spellcheck: "false",
  });
  const list = h("ul", { id: `${id}-list`, role: "listbox", class: "results", hidden: true, "aria-label": opts.label });
  const status = h("span", { class: "sr-only", role: "status", "aria-live": "polite" });
  const root = h("div", { class: "searchbox" }, h("label", { for: id, class: "sr-only" }, opts.label), input, list, status);
  let hits: SearchHit[] = [];
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
          hit.matched.field !== "label"
            ? h("span", { class: "hit-match" }, h("span", { class: "field" }, hit.matched.field),
              h("span", { class: hit.matched.field === "formula" ? "formula-src" : "" }, hit.matched.text.slice(0, 90)))
            : null,
        ),
      ),
    );
    if (!hits.length) list.append(h("li", { class: "empty", role: "option", "aria-disabled": "true" }, t("noResults")));
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
      const r = await api.search(q, getLang(), opts.limit ?? 12, ctrl.signal);
      hits = r.hits;
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
  return { root, input, setValue: (label: string) => { input.value = label; } };
}
