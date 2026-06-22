import type { Editor } from "../wasm/vellum.js";
import { computeDiff } from "./diff.js";
import { instanceHighlights } from "./highlight-names.js";
import { groupTokensByKind } from "./highlights.js";
import { createInputSource } from "./input/create-input-source.js";
import { CachingMeasurePort, canvasMeasure } from "./measure.js";

// Monotonic source of per-instance ids, so each surface gets disjoint highlight
// names within this module instance. Two *separately bundled* vellum copies on one
// page would each start at 0 and could mint the same id — acceptable for the engine's
// single-bundle use; swap for a random id if true cross-bundle isolation is needed.
let instanceSeq = 0;

/**
 * Mount a Vellum editor surface into `host`.
 *
 * Design:
 * - `.vellum-surface` is the scroll viewport; inside it `.vellum-content` is a
 *   spacer sized to the full document height (`lineCount × lineHeight`), and only
 *   the **visible** lines exist in the DOM as absolutely-positioned `.vellum-line`
 *   elements (virtualization, ADR-0004). The visible window comes from the core
 *   (`editor.visible_lines`); the view never reads layout from the DOM.
 * - Font metrics are measured **once** via the `MeasurePort` (Canvas, no reflow)
 *   and reused for every render and scroll.
 * - Input is captured through the `InputSource` port (ADR-0003), chosen by feature
 *   detection: `EditContextInput` on Chromium, else a hidden `HiddenTextareaInput`.
 * - Highlighting is painted via the CSS Custom Highlight API — zero `<span>` per
 *   token. Each visible line asks the core for its tokens in **line-local UTF-16**
 *   (`editor.tokens_in_line`) and registers `Range`s over that line's text node.
 *   Names are scoped per instance (blocker #2) so coexisting surfaces never clobber
 *   each other in the global `CSS.highlights` registry.
 *
 * Lines do not wrap (`white-space: pre`): one logical line is one row, which keeps
 * the monospace grid (`row = line × lineHeight`) exact for virtualization and the
 * caret. Long lines scroll horizontally.
 *
 * Each input change is applied as the **minimal diff** between the core's current
 * text and the device's new value (one `delete` + one `insert`; Increment-1
 * blocker #1).
 *
 * The caller owns the returned disposer: re-mounting into the same `host` without
 * calling it first leaks the previous instance's `<style>` and `CSS.highlights`
 * entries (this `replaceChildren` only clears the host's DOM children).
 *
 * @returns a disposer that releases the input source, listeners, highlights, and style.
 */
export function mountVellum(host: HTMLElement, editor: Editor): () => void {
  host.replaceChildren();

  const surface = document.createElement("div");
  surface.className = "vellum-surface";
  const content = document.createElement("div");
  content.className = "vellum-content";
  surface.appendChild(content);
  host.appendChild(surface);

  // Instance-scoped highlight names + their `::highlight()` rules (blocker #2).
  const { nameByKind, styleText } = instanceHighlights(String(instanceSeq++));
  const styleEl = document.createElement("style");
  styleEl.textContent = styleText;
  (document.head ?? document.documentElement).appendChild(styleEl);

  // Measure the font once (ADR-0004); all layout is arithmetic over these metrics.
  const measure = new CachingMeasurePort(canvasMeasure(surface));

  // Pick the input adapter by feature detection; the view holds only the port.
  const input = createInputSource(host, surface, editor.text());

  // The device value the core was last synced to, held to diff each change
  // against. Starts equal to the core text; updated after every applied edit (and,
  // later, after programmatic pushes via `input.setValue` on undo/redo).
  let lastValue = editor.text();

  const render = (): void => {
    renderViewport(editor, surface, content, measure, nameByKind);
  };

  input.onChange((change) => {
    applyDiff(editor, lastValue, change.value);
    lastValue = change.value;
    render();
  });
  // Re-window on scroll: only the visible lines are ever in the DOM.
  surface.addEventListener("scroll", render, { passive: true });

  render();
  input.focus();

  return () => {
    surface.removeEventListener("scroll", render);
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
 * Render the visible window: size the spacer to the whole document, ask the core
 * which lines are in view for the current scroll, and rebuild only those lines'
 * DOM + highlights. Off-screen lines never touch the DOM.
 */
function renderViewport(
  editor: Editor,
  surface: HTMLElement,
  content: HTMLElement,
  measure: CachingMeasurePort,
  nameByKind: Record<number, string>,
): void {
  const { lineHeight } = measure.metrics();
  const lineCount = editor.line_count();
  content.style.height = `${lineCount * lineHeight}px`;

  // TODO(I5): re-window on resize too (ResizeObserver). Until then a host that is
  // resized — or laid out (clientHeight 0) after mount — only re-windows on the
  // next scroll/keystroke.
  const win = editor.visible_lines(surface.scrollTop, surface.clientHeight, lineHeight);
  const start = win[0] ?? 0;
  const end = win[1] ?? 0;

  content.replaceChildren();
  const rangesByKind: Record<number, Range[]> = {};

  for (let line = start; line < end; line += 1) {
    const lineEl = document.createElement("div");
    lineEl.className = "vellum-line";
    lineEl.style.top = `${line * lineHeight}px`;
    // Per-line text from the core, so the view never owns a second line model
    // (its split would diverge from ropey's line breaks for CR / U+2028 content).
    const textNode = document.createTextNode(editor.line_text(line));
    lineEl.appendChild(textNode);
    content.appendChild(lineEl);

    collectLineHighlights(editor.tokens_in_line(line), textNode, nameByKind, rangesByKind);
  }

  registerHighlights(rangesByKind, nameByKind);
}

/**
 * Turn a line's line-local UTF-16 token triples into DOM `Range`s over `textNode`,
 * accumulating them per kind across every visible line (one `Highlight` per kind
 * spans all visible lines). Token offsets are already UTF-16 (the core converted
 * them in `tokens_in_line`), so no byte mapping happens here.
 */
function collectLineHighlights(
  flat: Uint32Array,
  textNode: Text,
  nameByKind: Record<number, string>,
  rangesByKind: Record<number, Range[]>,
): void {
  const maxLen = textNode.data.length;
  for (const [kindStr, ranges] of Object.entries(groupTokensByKind(flat))) {
    const kind = Number(kindStr);
    if (nameByKind[kind] === undefined) continue;
    for (const [start, end] of ranges) {
      const s = Math.min(start, maxLen);
      const e = Math.min(end, maxLen);
      if (e <= s) continue;
      const range = document.createRange();
      range.setStart(textNode, s);
      range.setEnd(textNode, e);
      (rangesByKind[kind] ??= []).push(range);
    }
  }
}

/** Replace this instance's highlights with the freshly collected per-kind ranges. */
function registerHighlights(
  rangesByKind: Record<number, Range[]>,
  nameByKind: Record<number, string>,
): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  clearHighlights(nameByKind);
  for (const [kindStr, ranges] of Object.entries(rangesByKind)) {
    const name = nameByKind[Number(kindStr)];
    if (name !== undefined && ranges.length > 0) {
      CSS.highlights.set(name, new Highlight(...ranges));
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
