import type { Editor } from "../wasm/vellum.js";
import { groupTokensByKind } from "./highlights.js";

/**
 * Maps the core's `TokenKind` u32 to the CSS Custom Highlight registry name.
 * Kind 0 (Text) is intentionally absent — plain text is painted by the
 * surface's default color, not a highlight.
 */
const HIGHLIGHT_NAME_BY_KIND: Record<number, string> = {
  1: "vellum-variable",
  2: "vellum-statement",
  3: "vellum-comment",
};

/**
 * Mount a Vellum editor surface into `host`.
 *
 * Increment 0 design:
 * - The buffer text is rendered as a single text node inside `.vellum-surface`.
 * - A transparent `.vellum-input` textarea overlays the surface and is the
 *   `HiddenTextareaInput` adapter: it owns focus and captures typing.
 * - Highlighting is painted purely via the CSS Custom Highlight API — zero
 *   `<span>` per token — by building `Range`s over the surface text node.
 *
 * SHORTCUT (documented, Inc 0 only): on every `input` event we resync the core
 * to the textarea's full value via clear + reinsert, rather than diffing the
 * change. This is O(n) per keystroke and loses WASM event granularity, but it
 * is trivially correct and round-trips every edit through the Rust core.
 * Increment 1 replaces this with a real diff / EditContext adapter (ADR-0003).
 *
 * @returns a disposer that removes the mounted DOM and clears highlights.
 */
export function mountVellum(host: HTMLElement, editor: Editor): () => void {
  host.replaceChildren();

  const surface = document.createElement("div");
  surface.className = "vellum-surface";
  const textNode = document.createTextNode("");
  surface.appendChild(textNode);

  const input = document.createElement("textarea");
  input.className = "vellum-input";
  input.spellcheck = false;
  input.autocapitalize = "off";
  input.setAttribute("autocomplete", "off");
  input.setAttribute("autocorrect", "off");
  input.value = editor.text();

  host.appendChild(surface);
  host.appendChild(input);

  const render = (): void => {
    textNode.data = editor.text();
    paintHighlights(textNode, editor.tokens());
  };

  const onInput = (): void => {
    syncCoreToValue(editor, input.value);
    render();
  };

  input.addEventListener("input", onInput);

  render();

  return () => {
    input.removeEventListener("input", onInput);
    clearHighlights();
    host.replaceChildren();
  };
}

/** Resync the core buffer to `value` via clear + reinsert (Inc 0 shortcut). */
function syncCoreToValue(editor: Editor, value: string): void {
  const current = editor.text();
  if (current === value) return;
  // Core offsets are byte offsets, so delete the full current byte length.
  const currentBytes = new TextEncoder().encode(current).length;
  if (currentBytes > 0) {
    editor.delete(0, currentBytes);
  }
  if (value.length > 0) {
    editor.insert(0, value);
  }
}

/** Build per-kind `Range`s over `textNode` and register them as CSS highlights. */
function paintHighlights(textNode: Text, flat: Uint32Array): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;

  const groups = groupTokensByKind(flat);
  clearHighlights();

  // Core offsets are byte offsets; DOM `Range` offsets are UTF-16 code units.
  // For Increment 0 (ASCII-only demo content) these coincide, so we map
  // directly. Multibyte mapping is an Increment 1 concern (ADR-0001).
  const maxLen = textNode.data.length;

  for (const [kindStr, ranges] of Object.entries(groups)) {
    const name = HIGHLIGHT_NAME_BY_KIND[Number(kindStr)];
    if (name === undefined) continue;

    const domRanges: Range[] = [];
    for (const [start, end] of ranges) {
      const s = Math.min(start, maxLen);
      const e = Math.min(end, maxLen);
      if (e <= s) continue;
      const range = document.createRange();
      range.setStart(textNode, s);
      range.setEnd(textNode, e);
      domRanges.push(range);
    }
    if (domRanges.length > 0) {
      CSS.highlights.set(name, new Highlight(...domRanges));
    }
  }
}

/** Remove all Vellum highlights from the global registry. */
function clearHighlights(): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  for (const name of Object.values(HIGHLIGHT_NAME_BY_KIND)) {
    CSS.highlights.delete(name);
  }
}
