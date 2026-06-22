import type { Editor } from "../wasm/vellum.js";
import { computeDiff } from "./diff.js";
import { instanceHighlights } from "./highlight-names.js";
import { groupTokensByKind } from "./highlights.js";
import { createInputSource } from "./input/create-input-source.js";
import type { InputSource } from "./input/input-source.js";
import { applyMovement, historyIntent, isInertNavKey } from "./keyboard.js";
import { CachingMeasurePort, canvasMeasure } from "./measure.js";

// Monotonic source of per-instance ids, so each surface gets disjoint highlight
// names within this module instance. Two *separately bundled* vellum copies on one
// page would each start at 0 and could mint the same id — acceptable for the engine's
// single-bundle use; swap for a random id if true cross-bundle isolation is needed.
let instanceSeq = 0;

// Horizontal text inset; MUST match `.vellum-line { padding: 0 12px }` in
// highlight-styles.css so the caret (text-relative x from the core) lands on the glyph.
const LINE_PADDING_LEFT = 12;

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
  const caret = document.createElement("div");
  caret.className = "vellum-caret";

  // Instance-scoped highlight names + their `::highlight()` rules (blocker #2).
  const { nameByKind, selectionName, styleText } = instanceHighlights(String(instanceSeq++));
  const styleEl = document.createElement("style");
  styleEl.textContent = styleText;
  (document.head ?? document.documentElement).appendChild(styleEl);

  // Measure the font once (ADR-0004); all layout is arithmetic over these metrics.
  const measure = new CachingMeasurePort(canvasMeasure(surface));

  // Pick the input adapter by feature detection; the view holds only the port.
  const input = createInputSource(host, surface, editor.text());

  // The device value the core was last synced to, held to diff each change against.
  // Starts equal to the core text; updated after every applied edit and after the
  // programmatic pushes that undo/redo make via `applyProgrammatic` below.
  let lastValue = editor.text();

  const render = (): void => {
    renderViewport(editor, input, surface, content, caret, measure, nameByKind, selectionName);
  };

  input.onChange((change) => {
    applyDiff(editor, lastValue, change.value);
    lastValue = change.value;
    // The device knows where the caret landed after the edit; adopt it as the
    // core cursor (a positional insert/delete does not move the core selection).
    syncCursorFromDevice(editor, change);
    render();
  });

  // Push a programmatic core change (undo/redo) back to the device without echo,
  // re-sync the cursor, and repaint. Keeps `lastValue` the authority for diffing.
  const applyProgrammatic = (): void => {
    const text = editor.text();
    input.setValue(text);
    lastValue = text;
    syncDeviceFromCursor(editor, input);
    render();
  };

  // Caret/selection movement and undo/redo are owned by the core; the device
  // selection/value is kept in lockstep so the next edit lands correctly. Vertical
  // and line/page nav have no core mover yet, so they are swallowed (inert) rather
  // than left to drift the device caret out of sync with the core-owned caret.
  const onKeydown = (event: KeyboardEvent): void => {
    if (applyMovement(editor, event)) {
      event.preventDefault();
      syncDeviceFromCursor(editor, input);
      render();
      return;
    }
    const history = historyIntent(event);
    if (history !== null) {
      event.preventDefault();
      if (history === "undo" ? editor.undo() : editor.redo()) applyProgrammatic();
      return;
    }
    if (isInertNavKey(event)) event.preventDefault();
  };
  // Bind on `host`, not `surface`: the textarea fallback focuses a textarea that is
  // a sibling of `surface`, so keydown only reaches their common ancestor `host`.
  // (The EditContext adapter focuses `surface` itself; keydown bubbles to `host` too.)
  host.addEventListener("keydown", onKeydown);
  // Re-window on scroll: only the visible lines are ever in the DOM.
  surface.addEventListener("scroll", render, { passive: true });

  render();
  input.focus();

  return () => {
    host.removeEventListener("keydown", onKeydown);
    surface.removeEventListener("scroll", render);
    input.dispose();
    clearAllHighlights(nameByKind, selectionName);
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
 * Adopt the device's reported selection as the core cursor (UTF-16 → byte). Called
 * after an edit, because a positional `insert`/`delete` does not move the core
 * selection — the device is the authority on where the caret landed.
 */
function syncCursorFromDevice(editor: Editor, change: { selectionStart: number; selectionEnd: number }): void {
  const anchor = editor.utf16_to_byte(change.selectionStart);
  const head = editor.utf16_to_byte(change.selectionEnd);
  editor.set_selection(anchor, head);
}

/**
 * Push the core cursor back to the device (byte → UTF-16) after a keyboard move.
 * Direction (which end is the head) is deliberately dropped — the device selection
 * only feeds the next edit; the caret/selection render reads the core, which keeps
 * direction. Do not "fix" this to preserve direction; it would change nothing for Inc-1.
 */
function syncDeviceFromCursor(editor: Editor, input: { setSelection(start: number, end: number): void }): void {
  const anchor = editor.byte_to_utf16(editor.cursor_anchor());
  const head = editor.byte_to_utf16(editor.cursor_head());
  input.setSelection(Math.min(anchor, head), Math.max(anchor, head));
}

/**
 * Render the visible window: size the spacer to the whole document, ask the core
 * which lines are in view for the current scroll, and rebuild only those lines'
 * DOM, highlights, and selection. Then position the caret. Off-screen lines never
 * touch the DOM.
 */
function renderViewport(
  editor: Editor,
  input: InputSource,
  surface: HTMLElement,
  content: HTMLElement,
  caret: HTMLElement,
  measure: CachingMeasurePort,
  nameByKind: Record<number, string>,
  selectionName: string,
): void {
  const { advance, lineHeight } = measure.metrics();
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
  const selectionRanges: Range[] = [];

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
    collectLineSelection(editor.selection_in_line(line), textNode, selectionRanges);
  }

  registerHighlights(rangesByKind, nameByKind);
  registerSelection(selectionRanges, selectionName);
  positionCaret(caret, content, editor, advance, lineHeight);
  // Push caret bounds to the device so an IME positions its candidate window here.
  // NOTE: this runs on every render (including scroll), so the two getBoundingClientRect
  // reads are a per-frame reflow — acceptable for Inc-1; TODO(perf) compute the screen
  // rect arithmetically (cached surface origin + caret_xy − scroll) to drop the reflow.
  input.updateCaretBounds(surface.getBoundingClientRect(), caret.getBoundingClientRect());
}

/** Build a `Range` over `textNode` for the line-local selection span, if any. */
function collectLineSelection(sel: Uint32Array, textNode: Text, out: Range[]): void {
  if (sel.length < 2) return;
  const maxLen = textNode.data.length;
  const s = Math.min(sel[0] ?? 0, maxLen);
  const e = Math.min(sel[1] ?? 0, maxLen);
  if (e <= s) return;
  const range = document.createRange();
  range.setStart(textNode, s);
  range.setEnd(textNode, e);
  out.push(range);
}

/** Position the caret at the core head (text-relative x, plus the line inset). */
function positionCaret(
  caret: HTMLElement,
  content: HTMLElement,
  editor: Editor,
  advance: number,
  lineHeight: number,
): void {
  const xy = editor.caret_xy(advance, lineHeight);
  caret.style.left = `${LINE_PADDING_LEFT + (xy[0] ?? 0)}px`;
  caret.style.top = `${xy[1] ?? 0}px`;
  caret.style.height = `${lineHeight}px`;
  content.appendChild(caret);
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

/** Replace this instance's selection highlight with the collected per-line ranges. */
function registerSelection(ranges: Range[], selectionName: string): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  CSS.highlights.delete(selectionName);
  if (ranges.length > 0) {
    CSS.highlights.set(selectionName, new Highlight(...ranges));
  }
}

/** Remove this instance's token highlights (only its `nameByKind`) from the registry. */
function clearHighlights(nameByKind: Record<number, string>): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  for (const name of Object.values(nameByKind)) {
    CSS.highlights.delete(name);
  }
}

/** Remove all of this instance's highlights (tokens + selection) on dispose. */
function clearAllHighlights(nameByKind: Record<number, string>, selectionName: string): void {
  if (typeof CSS === "undefined" || !("highlights" in CSS)) return;
  clearHighlights(nameByKind);
  CSS.highlights.delete(selectionName);
}
