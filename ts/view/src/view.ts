import type { Editor } from "../wasm/vellum.js";
import { groupTokensByKind } from "./highlights.js";
import { createInputSource } from "./input/create-input-source.js";

/**
 * Maps the core's `HighlightKind` u32 (ADR-0009) to the CSS Custom Highlight
 * registry name. Kind 0 (Text) is intentionally absent — plain text is painted
 * by the surface's default color, not a highlight.
 */
const HIGHLIGHT_NAME_BY_KIND: Record<number, string> = {
  1: "vellum-variable",
  2: "vellum-keyword",
  3: "vellum-comment",
};

/**
 * Mount a Vellum editor surface into `host`.
 *
 * Design:
 * - The buffer text is rendered as a single text node inside `.vellum-surface`.
 * - Input is captured through the `InputSource` port (ADR-0003), chosen by
 *   feature detection: `EditContextInput` on Chromium, else a hidden
 *   `HiddenTextareaInput` overlay. The view never knows which is active.
 * - Highlighting is painted purely via the CSS Custom Highlight API — zero
 *   `<span>` per token — by building `Range`s over the surface text node.
 *
 * SHORTCUT (documented, until Task I2): on every input change we resync the core
 * to the device's full value via clear + reinsert, rather than diffing. This is
 * O(n) per keystroke and loses WASM event granularity, but is trivially correct
 * and round-trips every edit through the Rust core. Task I2 replaces it with
 * diff-based mutation (UTF-16 → byte conversion via the wasm helpers).
 *
 * @returns a disposer that releases the input source and clears highlights.
 */
export function mountVellum(host: HTMLElement, editor: Editor): () => void {
  host.replaceChildren();

  const surface = document.createElement("div");
  surface.className = "vellum-surface";
  const textNode = document.createTextNode("");
  surface.appendChild(textNode);
  host.appendChild(surface);

  // Pick the input adapter by feature detection; the view holds only the port.
  const input = createInputSource(host, surface, editor.text());

  const render = (): void => {
    textNode.data = editor.text();
    paintHighlights(textNode, editor.tokens());
  };

  input.onChange((change) => {
    syncCoreToValue(editor, change.value);
    render();
  });

  render();
  input.focus();

  return () => {
    input.dispose();
    clearHighlights();
    host.replaceChildren();
  };
}

/** Resync the core buffer to `value` via clear + reinsert (Inc 0 shortcut). */
function syncCoreToValue(editor: Editor, value: string): void {
  const current = editor.text();
  if (current === value) return;
  // Core offsets are UTF-8 byte offsets; `delete`/`insert` PANIC (an
  // unrecoverable WASM trap) on a non-char-boundary offset. The ONLY thing
  // keeping this char-boundary-safe is that we always delete the FULL buffer
  // [0, currentBytes] and insert at 0 — both are guaranteed boundaries.
  // ⚠️ Increment 1 (ADR-0003) replaces this with diff-based mutation: it MUST
  // convert UTF-16 (DOM/textarea) offsets to UTF-8 byte offsets before calling
  // delete/insert, or the core will trap. Do not remove the full-buffer delete
  // without doing that conversion first.
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

// ⚠️ `CSS.highlights` is a process-global singleton and these highlight names
// are fixed strings, so Vellum currently assumes a SINGLE mounted surface. Two
// coexisting `mountVellum` instances would clobber each other's ranges globally.
// Increment 1 multi-surface support must namespace these names per instance.
/** Remove all Vellum highlights from the global registry. */
function clearHighlights(): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  for (const name of Object.values(HIGHLIGHT_NAME_BY_KIND)) {
    CSS.highlights.delete(name);
  }
}
