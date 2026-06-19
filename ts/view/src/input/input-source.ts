/**
 * The `InputSource` port (ADR-0003): the single outbound input abstraction the
 * view reads characters/composition from, so it never hard-codes a browser input
 * mechanism. Three adapters implement it — `EditContextInput` (Chromium native
 * IME), `HiddenTextareaInput` (the everywhere fallback), and `FakeInput` (tests,
 * no DOM). The view is written once against this contract; feature detection at
 * the boundary picks the adapter.
 *
 * **Not frozen — composition-incomplete by design (until Task I5).** The Inc-1
 * port covers value/selection edits (enough for Task I2's diff-based mutation).
 * IME composition state and pushing character/control bounds back to the device
 * (so the candidate window positions) are a deliberate Task-I5 addition that will
 * *widen* this interface and all three adapters. Treat it as evolving until then.
 */

/**
 * A snapshot of an input device's state, in **UTF-16 code units** (DOM space) —
 * the coordinate space the DOM, `<textarea>`, and `EditContext` all speak. The
 * view converts these to UTF-8 byte offsets (via the wasm `utf16_to_byte` /
 * `byte_to_utf16` helpers) before touching the core, which is byte-indexed
 * (ADR-0001; Increment-1 blocker #1). Diffing two `InputChange`s is how Task I2
 * derives a minimal edit.
 */
export interface InputChange {
  /** The device's full current text. */
  readonly value: string;
  /** Selection start in UTF-16 code units. */
  readonly selectionStart: number;
  /** Selection end in UTF-16 code units. */
  readonly selectionEnd: number;
}

/** Listener for user-driven input changes. */
export type InputListener = (change: InputChange) => void;

/**
 * The outbound input port. The view subscribes via [`onChange`](InputSource.onChange)
 * for user-driven edits and pushes canonical state back with
 * [`setValue`](InputSource.setValue) / [`setSelection`](InputSource.setSelection)
 * after programmatic edits (undo/redo). Increment 1 supports a single listener.
 */
export interface InputSource {
  /** The device's current state snapshot. */
  readonly state: InputChange;
  /** Subscribe to user-driven changes (the device's value/selection changed). */
  onChange(listener: InputListener): void;
  /** Push canonical text into the device after a programmatic edit (no echo). */
  setValue(value: string): void;
  /** Move the device selection (UTF-16 code units). */
  setSelection(start: number, end: number): void;
  /** Give the device input focus. */
  focus(): void;
  /** Detach listeners / DOM and release the device. */
  dispose(): void;
}
