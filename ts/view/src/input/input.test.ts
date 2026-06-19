import { describe, it, expect } from "vitest";
import { FakeInput } from "./fake-input.js";
import type { InputChange } from "./input-source.js";
import { supportsEditContext } from "./create-input-source.js";

describe("FakeInput", () => {
  it("starts with the initial value and a collapsed caret at the end", () => {
    const input = new FakeInput("hello");
    expect(input.state).toEqual({
      value: "hello",
      selectionStart: 5,
      selectionEnd: 5,
    });
  });

  it("type() inserts at the caret, advances it, and emits the new state", () => {
    const input = new FakeInput("ac");
    input.setSelection(1, 1);
    const changes: InputChange[] = [];
    input.onChange((c) => changes.push(c));

    input.type("b");

    expect(input.state).toEqual({
      value: "abc",
      selectionStart: 2,
      selectionEnd: 2,
    });
    expect(changes.at(-1)).toEqual({
      value: "abc",
      selectionStart: 2,
      selectionEnd: 2,
    });
  });

  it("type() over a non-empty selection replaces it", () => {
    const input = new FakeInput("aXXc");
    input.setSelection(1, 3);
    input.type("b");
    expect(input.state.value).toBe("abc");
    expect(input.state.selectionStart).toBe(2);
    expect(input.state.selectionEnd).toBe(2);
  });

  it("type() works in UTF-16 code units across an astral char", () => {
    // "a😀c": 😀 is 2 UTF-16 code units, so it spans indices 1..3.
    const input = new FakeInput("a😀c");
    input.setSelection(1, 3); // select the emoji
    input.type("X");
    expect(input.state.value).toBe("aXc");
  });

  it("setValue() replaces the value and clamps the selection without emitting", () => {
    const input = new FakeInput("hello");
    input.setSelection(4, 5);
    let emitted = false;
    input.onChange(() => {
      emitted = true;
    });

    input.setValue("hi"); // shorter than the old selection

    expect(input.state.value).toBe("hi");
    expect(input.state.selectionStart).toBe(2); // clamped to new length
    expect(input.state.selectionEnd).toBe(2);
    expect(emitted).toBe(false); // programmatic push is not a user change
  });

  it("setSelection() clamps to the value bounds", () => {
    const input = new FakeInput("hi");
    input.setSelection(99, 99);
    expect(input.state.selectionStart).toBe(2);
    expect(input.state.selectionEnd).toBe(2);
  });

  it("dispose() stops delivering changes to the listener", () => {
    const input = new FakeInput("");
    let count = 0;
    input.onChange(() => {
      count += 1;
    });
    input.type("a");
    input.dispose();
    input.type("b");
    expect(count).toBe(1);
  });
});

describe("supportsEditContext", () => {
  it("is true when the global exposes EditContext", () => {
    expect(supportsEditContext({ EditContext: class {} })).toBe(true);
  });

  it("is false when the global lacks EditContext (Safari/Firefox)", () => {
    expect(supportsEditContext({})).toBe(false);
  });
});
