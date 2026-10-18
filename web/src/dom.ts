// Minimal DOM builder.  Text always goes through textContent (no innerHTML with
// data), so nothing from the registry can inject markup.

type Child = Node | string | number | null | undefined | false;
type Attrs = Record<string, string | number | boolean | EventListener | undefined>;

export function h<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Attrs = {}, ...children: (Child | Child[])[]): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === undefined || v === false) continue;
    if (k.startsWith("on") && typeof v === "function") el.addEventListener(k.slice(2), v);
    else if (k === "class") el.className = String(v);
    else el.setAttribute(k, v === true ? "" : String(v));
  }
  for (const c of children.flat()) {
    if (c === null || c === undefined || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

/** A link only for http(s) URIs; anything else is shown as text. */
export function safeLink(uri: string | null | undefined, text: string): Node {
  if (uri && /^https?:\/\//i.test(uri)) return h("a", { href: uri, target: "_blank", rel: "noopener noreferrer" }, text);
  return document.createTextNode(text);
}

export function clear(el: Element): void {
  el.replaceChildren();
}
