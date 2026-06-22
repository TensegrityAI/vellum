import type { Editor } from "../wasm/vellum.js";
import { computeDiff } from "./diff.js";
import { instanceHighlights } from "./highlight-names.js";
import { groupTokensByKind } from "./highlights.js";
import { createInputSource } from "./input/create-input-source.js";

// Monotonic source of per-instance ids, so each surface gets disjoint highlight
// names within this module instance. Two *separately bundled* vellum copies on one
// page would each start at 0 and could mint the same id — acceptable for the engine's
// single-bundle use; swap for a random id if true cross-bundle isolation is needed.
let instanceSeq = 0;

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
 *   Names are scoped per instance (blocker #2) so multiple surfaces coexisting on
 *   one page never clobber each other in the global `CSS.highlights` registry.
 *
 * Each input change is applied as the **minimal diff** between the core's current
 * text and the device's new value: one `delete` + one `insert` over only the
 * edited run, with UTF-16 (DOM) offsets converted to UTF-8 byte offsets via the
 * wasm helper before touching the byte-indexed core (Increment-1 blocker #1).
 *
 * The caller owns the returned disposer: re-mounting into the same `host` without
 * calling it first leaks the previous instance's `<style>` and `CSS.highlights`
 * entries (this `replaceChildren` only clears the host's DOM children).
 *
 * @returns a disposer that releases the input source, highlights, and style.
 */
export function mountVellum(host: HTMLElement, editor: Editor): () => void {
  host.replaceChildren();

  const surface = document.createElement("div");
  surface.className = "vellum-surface";
  const textNode = document.createTextNode("");
  surface.appendChild(textNode);
  host.appendChild(surface);

  // Instance-scoped highlight names + their `::highlight()` rules (blocker #2).
  const { nameByKind, styleText } = instanceHighlights(String(instanceSeq++));
  const styleEl = document.createElement("style");
  styleEl.textContent = styleText;
  (document.head ?? document.documentElement).appendChild(styleEl);

  // Pick the input adapter by feature detection; the view holds only the port.
  const input = createInputSource(host, surface, editor.text());

  // The device value the core was last synced to, held to diff each change
  // against. Starts equal to the core text; updated after every applied edit (and,
  // later, after programmatic pushes via `input.setValue` on undo/redo).
  let lastValue = editor.text();

  const render = (): void => {
    textNode.data = editor.text();
    paintHighlights(textNode, editor.tokens(), nameByKind, (b) => editor.byte_to_utf16(b));
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
    clearHighlights(nameByKind);
    styleEl.remove();
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

/**
 * Build per-kind `Range`s over `textNode` and register them under this instance's
 * `nameByKind` in `CSS.highlights`.
 */
function paintHighlights(
  textNode: Text,
  flat: Uint32Array,
  nameByKind: Record<number, string>,
  byteToUtf16: (byteOffset: number) => number,
): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;

  const groups = groupTokensByKind(flat);
  clearHighlights(nameByKind);

  // Token offsets are UTF-8 byte offsets (core space); DOM `Range` offsets are
  // UTF-16 code units. Convert through the core's `byte_to_utf16` so multibyte
  // text (emoji/CJK) highlights at the right place (ADR-0001). Token offsets come
  // from the core's own buffer, so they are in-range char boundaries (no trap).
  const maxLen = textNode.data.length;

  for (const [kindStr, ranges] of Object.entries(groups)) {
    const name = nameByKind[Number(kindStr)];
    if (name === undefined) continue;

    const domRanges: Range[] = [];
    for (const [start, end] of ranges) {
      const s = Math.min(byteToUtf16(start), maxLen);
      const e = Math.min(byteToUtf16(end), maxLen);
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

/** Remove this instance's highlights (only its `nameByKind`) from the global registry. */
function clearHighlights(nameByKind: Record<number, string>): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  for (const name of Object.values(nameByKind)) {
    CSS.highlights.delete(name);
  }
}
