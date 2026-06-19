import type { InputChange, InputListener, InputSource } from "./input-source.js";

/**
 * In-memory [`InputSource`](./input-source.ts) for tests (ADR-0003): inject
 * typing and selection with **no DOM**, so input behavior is fast, deterministic,
 * and agent-driven to test. It operates in UTF-16 code units via JS string
 * indices — exactly the space a real `<textarea>` reports — so a `FakeInput`
 * keystroke and a textarea keystroke produce the same `InputChange`.
 *
 * The `type`/`setSelection`/`setValue`/`onChange` surface mirrors the contract a
 * DOM adapter satisfies; `type` is the test driver that simulates a user
 * keystroke (replace selection, advance caret, emit a change).
 */
export class FakeInput implements InputSource {
  #value: string;
  #start: number;
  #end: number;
  #listener: InputListener | null = null;

  constructor(initial = "") {
    this.#value = initial;
    this.#start = initial.length;
    this.#end = initial.length;
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
    // Programmatic push from the view (e.g. after undo). Mirrors assigning
    // `textarea.value`: it updates state but does NOT echo back as a user change.
    this.#value = value;
    this.#clampSelection();
  }

  setSelection(start: number, end: number): void {
    this.#start = start;
    this.#end = end;
    this.#clampSelection();
  }

  focus(): void {
    // No-op: focus is meaningless without a DOM device. Kept to satisfy the port.
  }

  dispose(): void {
    this.#listener = null;
  }

  // --- Test driver: simulate the user -----------------------------------

  /**
   * Simulate typing `text` at the caret, replacing any non-empty selection, then
   * emit the resulting [`InputChange`] — exactly what a `<textarea>` `input`
   * event would surface. Indices are UTF-16 code units (JS string slicing).
   */
  type(text: string): void {
    const lo = Math.min(this.#start, this.#end);
    const hi = Math.max(this.#start, this.#end);
    this.#value = this.#value.slice(0, lo) + text + this.#value.slice(hi);
    const caret = lo + text.length;
    this.#start = caret;
    this.#end = caret;
    this.#emit();
  }

  #clampSelection(): void {
    const len = this.#value.length;
    this.#start = Math.max(0, Math.min(this.#start, len));
    this.#end = Math.max(0, Math.min(this.#end, len));
  }

  #emit(): void {
    this.#listener?.(this.state);
  }
}
