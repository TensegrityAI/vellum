import type { InputChange, InputListener, InputSource } from "./input-source.js";

/**
 * The everywhere-fallback [`InputSource`](./input-source.ts) (ADR-0003): a hidden,
 * transparent `<textarea>` overlay that owns focus and captures typing — the
 * proven CodeMirror pattern. Works on every browser with correct IME and
 * accessibility. This is the Increment-0 textarea, now behind the port.
 *
 * DOM wiring is verified in the demo / under jsdom (Phase J), not the node-env
 * unit tests; the behavior contract it satisfies is exercised via [`FakeInput`].
 */
export class HiddenTextareaInput implements InputSource {
  readonly #textarea: HTMLTextAreaElement;
  #listener: InputListener | null = null;
  readonly #onInput: () => void;

  constructor(host: HTMLElement, initial = "") {
    const ta = document.createElement("textarea");
    ta.className = "vellum-input";
    ta.spellcheck = false;
    ta.autocapitalize = "off";
    ta.setAttribute("autocomplete", "off");
    ta.setAttribute("autocorrect", "off");
    ta.value = initial;
    host.appendChild(ta);
    this.#textarea = ta;

    this.#onInput = (): void => this.#listener?.(this.state);
    ta.addEventListener("input", this.#onInput);
  }

  get state(): InputChange {
    return {
      value: this.#textarea.value,
      // A textarea's selection is null only when detached; default to the end.
      selectionStart: this.#textarea.selectionStart ?? this.#textarea.value.length,
      selectionEnd: this.#textarea.selectionEnd ?? this.#textarea.value.length,
    };
  }

  onChange(listener: InputListener): void {
    this.#listener = listener;
  }

  setValue(value: string): void {
    // Assigning `.value` does not fire `input`, so this stays a non-echoing
    // programmatic push (matches the port contract).
    this.#textarea.value = value;
  }

  setSelection(start: number, end: number): void {
    this.#textarea.setSelectionRange(start, end);
  }

  updateCaretBounds(): void {
    // No-op: the textarea owns its own (invisible) caret, and the browser positions
    // the IME candidate window at it; the overlay needs no explicit bounds push.
  }

  focus(): void {
    this.#textarea.focus();
  }

  dispose(): void {
    this.#textarea.removeEventListener("input", this.#onInput);
    this.#textarea.remove();
    this.#listener = null;
  }
}
