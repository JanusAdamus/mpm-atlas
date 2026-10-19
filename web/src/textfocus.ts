// Focus when switching between the map and the concept index (R4-04): opening the
// index moves focus to its heading, so the next Tab reaches its first link; going
// back to the map returns focus to where the reader was (or to the map).

export interface Focusable {
  focus(options?: { preventScroll?: boolean }): void;
  isConnected?: boolean;
}

export function focusReturn(active: () => Focusable | null) {
  let back: Focusable | null = null;
  return {
    /** Remember the current focus and move it to `target`. */
    open(target: Focusable): void {
      back = active();
      target.focus();
    },
    /** Return focus to where it was before `open`, else to `fallback`. */
    close(fallback: Focusable): void {
      const to = back && back.isConnected !== false ? back : fallback;
      back = null;
      to.focus();
    },
  };
}
