import { describe, it, expect } from "vitest";
import { applyMovement, historyIntent, isInertNavKey } from "./keyboard.js";
import type { MovementEditor } from "./keyboard.js";

/** Records which mover the keyboard layer invoked, so we test the policy, not a DOM. */
function recordingEditor(): MovementEditor & { calls: string[] } {
  const calls: string[] = [];
  const rec =
    (name: string) =>
    (): void => {
      calls.push(name);
    };
  return {
    calls,
    move_left: rec("move_left"),
    move_right: rec("move_right"),
    extend_left: rec("extend_left"),
    extend_right: rec("extend_right"),
    move_word_left: rec("move_word_left"),
    move_word_right: rec("move_word_right"),
    extend_word_left: rec("extend_word_left"),
    extend_word_right: rec("extend_word_right"),
  };
}

const key = (
  k: string,
  mods: { shiftKey?: boolean; ctrlKey?: boolean; metaKey?: boolean; altKey?: boolean } = {},
): { key: string; shiftKey: boolean; ctrlKey: boolean; metaKey: boolean; altKey: boolean } => ({
  key: k,
  shiftKey: mods.shiftKey ?? false,
  ctrlKey: mods.ctrlKey ?? false,
  metaKey: mods.metaKey ?? false,
  altKey: mods.altKey ?? false,
});

describe("applyMovement", () => {
  it("ArrowRight moves one grapheme right", () => {
    const ed = recordingEditor();
    expect(applyMovement(ed, key("ArrowRight"))).toBe(true);
    expect(ed.calls).toEqual(["move_right"]);
  });

  it("Shift+ArrowLeft extends the selection left", () => {
    const ed = recordingEditor();
    applyMovement(ed, key("ArrowLeft", { shiftKey: true }));
    expect(ed.calls).toEqual(["extend_left"]);
  });

  it("Ctrl+ArrowRight moves one word right", () => {
    const ed = recordingEditor();
    applyMovement(ed, key("ArrowRight", { ctrlKey: true }));
    expect(ed.calls).toEqual(["move_word_right"]);
  });

  it("Ctrl+Shift+ArrowLeft extends by a word", () => {
    const ed = recordingEditor();
    applyMovement(ed, key("ArrowLeft", { ctrlKey: true, shiftKey: true }));
    expect(ed.calls).toEqual(["extend_word_left"]);
  });

  it("ignores Alt/Meta combos (returns false, no mover)", () => {
    const ed = recordingEditor();
    expect(applyMovement(ed, key("ArrowRight", { altKey: true }))).toBe(false);
    expect(applyMovement(ed, key("ArrowLeft", { metaKey: true }))).toBe(false);
    expect(ed.calls).toEqual([]);
  });

  it("does not handle non-arrow keys", () => {
    const ed = recordingEditor();
    expect(applyMovement(ed, key("a"))).toBe(false);
    expect(ed.calls).toEqual([]);
  });
});

describe("historyIntent", () => {
  it("Ctrl+Z is undo", () => {
    expect(historyIntent(key("z", { ctrlKey: true }))).toBe("undo");
  });
  it("Cmd+Z is undo (macOS)", () => {
    expect(historyIntent(key("z", { metaKey: true }))).toBe("undo");
  });
  it("Ctrl+Y is redo", () => {
    expect(historyIntent(key("y", { ctrlKey: true }))).toBe("redo");
  });
  it("Ctrl+Shift+Z is redo", () => {
    expect(historyIntent(key("z", { ctrlKey: true, shiftKey: true }))).toBe("redo");
  });
  it("a bare 'z' is not a history command", () => {
    expect(historyIntent(key("z"))).toBeNull();
  });
});

describe("isInertNavKey", () => {
  it("vertical and line-nav keys are inert (no core mover yet)", () => {
    for (const k of ["ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]) {
      expect(isInertNavKey(key(k))).toBe(true);
    }
  });
  it("horizontal arrows and letters are not inert", () => {
    expect(isInertNavKey(key("ArrowLeft"))).toBe(false);
    expect(isInertNavKey(key("a"))).toBe(false);
  });
});
