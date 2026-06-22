import type { Editor } from "../wasm/vellum.js";
import { computeDiff } from "./diff.js";
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
 * Each input change is applied as the **minimal diff** between the core's current
 * text and the device's new value: one `delete` + one `insert` over only the
 * edited run, with UTF-16 (DOM) offsets converted to UTF-8 byte offsets via the
 * wasm helper before touching the byte-indexed core (Increment-1 blocker #1).
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

  // The device value the core was last synced to, held to diff each change
  // against. Starts equal to the core text; updated after every applied edit (and,
  // later, after programmatic pushes via `input.setValue` on undo/redo).
  let lastValue = editor.text();

  const render = (): void => {
    textNode.data = editor.text();
    paintHighlights(textNode, editor.tokens());
  };

  input.onChange((change) => {
    applyDiff(editor, lastValue, change.value);
    lastValue = change.value;
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

/**
 * Apply the minimal diff between the core's current text (`oldValue`) and the
 * device's `newValue` as one delete + one insert over the changed run. UTF-16
 * (DOM) offsets are converted to the core's UTF-8 byte offsets via the wasm
 * `utf16_to_byte` helper — both conversions happen against the unmutated core,
 * then the delete runs, then the insert at the same start. Offsets index
 * `oldValue`, which equals the core's current text, and the diff is surrogate-safe
 * and in-range, so they land on char boundaries the core accepts (no trap).
 *
 * `utf16_to_byte`/`delete`/`insert` are non-trapping but throw a JS `Error` on a
 * bad offset. They cannot throw here: well-formed UTF-16 — all a textarea or
 * `EditContext` can deliver — yields in-range, boundary-aligned offsets. A throw
 * would therefore signal a `lastValue`/core desync bug, not user input, so it is
 * intentionally left to surface rather than swallowed into a silent corruption.
 */
function applyDiff(editor: Editor, oldValue: string, newValue: string): void {
  const { utf16Start, utf16RemovedLen, inserted } = computeDiff(oldValue, newValue);
  if (utf16RemovedLen === 0 && inserted.length === 0) return; // no-op

  const byteStart = editor.utf16_to_byte(utf16Start);
  if (utf16RemovedLen > 0) {
    const byteEnd = editor.utf16_to_byte(utf16Start + utf16RemovedLen);
    editor.delete(byteStart, byteEnd);
  }
  if (inserted.length > 0) {
    editor.insert(byteStart, inserted);
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
