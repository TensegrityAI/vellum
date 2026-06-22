import type { InputChange, InputListener, InputSource, ScreenRect } from "./input-source.js";

// --- Minimal local typings for the EditContext API --------------------------
//
// EditContext is Chromium-only (Chrome/Edge 121+) and not yet in the standard TS
// DOM lib, so we declare the slice we use. Kept local to this adapter — the rest
// of the view never sees it (ADR-0003: the browser API stays behind the port).

interface EditContextTextUpdateEvent {
  readonly updateRangeStart: number;
  readonly updateRangeEnd: number;
  readonly text: string;
  readonly selectionStart: number;
  readonly selectionEnd: number;
}

interface EditContextLike {
  updateText(rangeStart: number, rangeEnd: number, text: string): void;
  updateSelection(start: number, end: number): void;
  updateControlBounds(controlBounds: DOMRect): void;
  updateSelectionBounds(selectionBounds: DOMRect): void;
  addEventListener(
    type: "textupdate",
    listener: (event: EditContextTextUpdateEvent) => void,
  ): void;
  removeEventListener(
    type: "textupdate",
    listener: (event: EditContextTextUpdateEvent) => void,
  ): void;
}

interface EditContextConstructor {
  new (options?: { text?: string }): EditContextLike;
}

/** The EditContext constructor if this is a Chromium that supports it, else null. */
function editContextCtor(): EditContextConstructor | null {
  const ctor = (globalThis as { EditContext?: EditContextConstructor }).EditContext;
  return ctor ?? null;
}

/**
 * The Chromium-native [`InputSource`](./input-source.ts) (ADR-0003): attaches an
 * `EditContext` to the surface element for native IME/composition — the best
 * input experience where available. Falls back to
 * [`HiddenTextareaInput`](./hidden-textarea-input.ts) elsewhere (the factory
 * decides via feature detection).
 *
 * Increment 1 handles `textupdate` (the device edited its text) and selection;
 * composition decoration (character bounds for the IME candidate window) is a
 * later refinement (Task I5). DOM/IME wiring is verified in the demo on Chromium,
 * not the node-env unit tests.
 *
 * **Assumes single ownership of `target`** (one adapter per element) and keeps a
 * local text mirror in lockstep with the EditContext buffer; `setValue` /
 * `setSelection` update both. A `textupdate` whose range the mirror didn't expect
 * (e.g. browser-coalesced composition) is a known Task-I5 concern, not handled
 * here.
 */
export class EditContextInput implements InputSource {
  readonly #target: HTMLElement;
  readonly #ec: EditContextLike;
  #value: string;
  #start: number;
  #end: number;
  #listener: InputListener | null = null;
  readonly #onTextUpdate: (event: EditContextTextUpdateEvent) => void;

  /**
   * @throws if EditContext is unavailable — callers must feature-detect first
   * (see [`supportsEditContext`](./create-input-source.ts)).
   */
  constructor(target: HTMLElement, initial = "") {
    const Ctor = editContextCtor();
    if (Ctor === null) {
      throw new Error("EditContext is not available in this environment");
    }
    this.#target = target;
    this.#value = initial;
    this.#start = initial.length;
    this.#end = initial.length;

    const ec = new Ctor({ text: initial });
    this.#ec = ec;
    // The element must be focusable to receive EditContext input.
    if (!target.hasAttribute("tabindex")) target.setAttribute("tabindex", "0");
    (target as unknown as { editContext: EditContextLike }).editContext = ec;

    this.#onTextUpdate = (event: EditContextTextUpdateEvent): void => {
      // Mirror the device's text model: splice the updated range, then adopt the
      // new selection. Offsets are UTF-16 code units, as the port requires.
      this.#value =
        this.#value.slice(0, event.updateRangeStart) +
        event.text +
        this.#value.slice(event.updateRangeEnd);
      this.#start = event.selectionStart;
      this.#end = event.selectionEnd;
      this.#listener?.(this.state);
    };
    ec.addEventListener("textupdate", this.#onTextUpdate);
  }

  get state(): InputChange {
    return {
      value: this.#value,
      selectionStart: this.#start,
      selectionEnd: this.#end,
    };
  }

  onChange(listener: InputListener): void {
    this.#listener = listener;
  }

  setValue(value: string): void {
    // Programmatic push (e.g. after undo): replace the whole EditContext text and
    // keep the mirror in sync. Does not echo back as a user change.
    this.#ec.updateText(0, this.#value.length, value);
    this.#value = value;
    if (this.#start > value.length) this.#start = value.length;
    if (this.#end > value.length) this.#end = value.length;
    this.#ec.updateSelection(this.#start, this.#end);
  }

  setSelection(start: number, end: number): void {
    this.#start = start;
    this.#end = end;
    this.#ec.updateSelection(start, end);
  }

  updateCaretBounds(control: ScreenRect, caret: ScreenRect): void {
    // Tell the IME where the editor and caret are so the candidate window appears
    // at the caret. `DOMRect.fromRect` is widely available where EditContext is.
    this.#ec.updateControlBounds(DOMRect.fromRect(control));
    this.#ec.updateSelectionBounds(DOMRect.fromRect(caret));
  }

  focus(): void {
    this.#target.focus();
  }

  dispose(): void {
    this.#ec.removeEventListener("textupdate", this.#onTextUpdate);
    (this.#target as unknown as { editContext: EditContextLike | null }).editContext = null;
    this.#listener = null;
  }
}
