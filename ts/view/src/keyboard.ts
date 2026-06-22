/**
 * Pure keyboard policy for the view: which key maps to which core intent. Kept
 * DOM-free (it reads only the modifier/key fields a `KeyboardEvent` exposes) so the
 * navigation/history contract is unit-tested without a browser — the view binds a
 * real `keydown` listener and delegates here.
 */

/** The subset of the core editor the movement layer drives (grapheme + word movers). */
export interface MovementEditor {
  move_left(): void;
  move_right(): void;
  extend_left(): void;
  extend_right(): void;
  move_word_left(): void;
  move_word_right(): void;
  extend_word_left(): void;
  extend_word_right(): void;
}

/** The modifier/key fields read off a `KeyboardEvent`. */
export interface KeyLike {
  readonly key: string;
  readonly shiftKey: boolean;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly altKey: boolean;
}

/**
 * Map a Left/Right arrow (optionally Ctrl for word, Shift to extend) onto the core
 * cursor. Returns whether it was a handled movement. Alt/Meta combos and non-arrow
 * keys are left for other handlers / the browser.
 */
export function applyMovement(editor: MovementEditor, event: KeyLike): boolean {
  if (event.altKey || event.metaKey) return false;
  const extend = event.shiftKey;
  const word = event.ctrlKey;
  switch (event.key) {
    case "ArrowLeft":
      if (extend && word) editor.extend_word_left();
      else if (extend) editor.extend_left();
      else if (word) editor.move_word_left();
      else editor.move_left();
      return true;
    case "ArrowRight":
      if (extend && word) editor.extend_word_right();
      else if (extend) editor.extend_right();
      else if (word) editor.move_word_right();
      else editor.move_right();
      return true;
    default:
      return false;
  }
}

/**
 * The undo/redo intent of a key, if any. Ctrl/Cmd+Z is undo; Ctrl+Y or
 * Ctrl/Cmd+Shift+Z is redo (covering both the Windows/Linux and macOS conventions).
 */
export function historyIntent(event: KeyLike): "undo" | "redo" | null {
  const accel = event.ctrlKey || event.metaKey;
  if (!accel) return null;
  const k = event.key.toLowerCase();
  if (k === "y") return "redo";
  if (k === "z") return event.shiftKey ? "redo" : "undo";
  return null;
}

/**
 * Whether `event` is a navigation key the core has no mover for yet (vertical and
 * line/page nav). The view swallows these so the browser cannot drift the device
 * caret out of sync with the core-owned caret; the movement itself is deferred.
 */
export function isInertNavKey(event: KeyLike): boolean {
  switch (event.key) {
    case "ArrowUp":
    case "ArrowDown":
    case "Home":
    case "End":
    case "PageUp":
    case "PageDown":
      return true;
    default:
      return false;
  }
}
