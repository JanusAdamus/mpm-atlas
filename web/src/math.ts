// KaTeX and its stylesheet, loaded on demand (the first panel with math) so the
// map's first paint does not wait for ~260 kB of typesetting code.
import katex from "katex";
import "katex/dist/katex.min.css";

export function renderMath(el: HTMLElement, tex: string, display: boolean): void {
  katex.render(tex, el, { throwOnError: false, displayMode: display, output: "htmlAndMathml", strict: "ignore", trust: false });
}
