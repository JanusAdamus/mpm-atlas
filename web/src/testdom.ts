// A tiny stand-in for the browser DOM, enough for `dom.ts` `h()` and the panels to
// render under `node --test`.  Test-only: nothing in the app imports it.

type Listener = (e: { type: string; target: unknown; preventDefault(): void }) => void;

export class FakeNode {
  childNodes: FakeNode[] = [];
  parentNode: FakeElement | null = null;
  get textContent(): string {
    return this.childNodes.map((c) => c.textContent).join("");
  }
  set textContent(v: string) {
    this.childNodes = [new FakeText(v)];
  }
  append(...nodes: (FakeNode | string)[]): void {
    for (const n of nodes) {
      const node = typeof n === "string" ? new FakeText(n) : n;
      node.parentNode = this as unknown as FakeElement;
      this.childNodes.push(node);
    }
  }
  replaceChildren(...nodes: (FakeNode | string)[]): void {
    this.childNodes = [];
    this.append(...nodes);
  }
}

export class FakeText extends FakeNode {
  readonly data: string;
  constructor(data: string) {
    super();
    this.data = data;
  }
  override get textContent(): string {
    return this.data;
  }
}

export class FakeElement extends FakeNode {
  readonly tagName: string;
  readonly attrs = new Map<string, string>();
  readonly listeners = new Map<string, Listener[]>();
  readonly dataset: Record<string, string> = {};
  className = "";
  title = "";
  focused = false;
  constructor(tag: string) {
    super();
    this.tagName = tag.toUpperCase();
  }
  override set textContent(v: string) {
    this.childNodes = [new FakeText(v)];
  }
  override get textContent(): string {
    return super.textContent;
  }
  setAttribute(k: string, v: string): void {
    this.attrs.set(k, v);
  }
  getAttribute(k: string): string | null {
    return this.attrs.get(k) ?? null;
  }
  removeAttribute(k: string): void {
    this.attrs.delete(k);
  }
  hasAttribute(k: string): boolean {
    return this.attrs.has(k);
  }
  get id(): string {
    return this.attrs.get("id") ?? "";
  }
  get hidden(): boolean {
    return this.attrs.has("hidden");
  }
  set hidden(v: boolean) {
    if (v) this.attrs.set("hidden", "");
    else this.attrs.delete("hidden");
  }
  get children(): FakeElement[] {
    return this.childNodes.filter((c): c is FakeElement => c instanceof FakeElement);
  }
  get childElementCount(): number {
    return this.children.length;
  }
  addEventListener(type: string, fn: Listener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), fn]);
  }
  dispatch(type: string): void {
    for (const fn of this.listeners.get(type) ?? []) fn({ type, target: this, preventDefault() {} });
  }
  click(): void {
    this.dispatch("click");
  }
  focus(): void {
    this.focused = true;
  }
}

/** Install the fake as `document` and `Node` (idempotent). */
export function installDom(): void {
  const g = globalThis as unknown as Record<string, unknown>;
  if (g.document) return;
  const documentElement = new FakeElement("html");
  g.Node = FakeNode;
  g.document = {
    documentElement,
    createElement: (tag: string) => new FakeElement(tag),
    createElementNS: (_ns: string, tag: string) => new FakeElement(tag),
    createTextNode: (s: string) => new FakeText(s),
  };
}

/** Every element under `root` (itself included) that matches `pred`, in document order. */
export function findAll(root: unknown, pred: (el: FakeElement) => boolean): FakeElement[] {
  const out: FakeElement[] = [];
  const walk = (n: FakeNode): void => {
    if (n instanceof FakeElement && pred(n)) out.push(n);
    n.childNodes.forEach(walk);
  };
  walk(root as FakeNode);
  return out;
}

export const byClass = (cls: string) => (el: FakeElement): boolean => el.className.split(/\s+/).includes(cls);
export const byTag = (tag: string) => (el: FakeElement): boolean => el.tagName === tag.toUpperCase();
export const textOf = (n: unknown): string => (n as FakeNode).textContent;
