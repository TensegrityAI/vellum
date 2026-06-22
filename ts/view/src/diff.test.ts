import { describe, it, expect } from "vitest";
import { computeDiff } from "./diff.js";

describe("computeDiff", () => {
  it("reports a no-op for equal strings", () => {
    const diff = computeDiff("hello", "hello");
    expect(diff).toEqual({ utf16Start: 5, utf16RemovedLen: 0, inserted: "" });
  });

  it("appends text at the end", () => {
    const diff = computeDiff("ab", "abc");
    expect(diff).toEqual({ utf16Start: 2, utf16RemovedLen: 0, inserted: "c" });
  });

  it("inserts text at the start", () => {
    const diff = computeDiff("bc", "abc");
    expect(diff).toEqual({ utf16Start: 0, utf16RemovedLen: 0, inserted: "a" });
  });

  it("inserts text in the middle", () => {
    const diff = computeDiff("ac", "abc");
    expect(diff).toEqual({ utf16Start: 1, utf16RemovedLen: 0, inserted: "b" });
  });

  it("deletes a single character in the middle", () => {
    const diff = computeDiff("abc", "ac");
    expect(diff).toEqual({ utf16Start: 1, utf16RemovedLen: 1, inserted: "" });
  });

  it("replaces a character in the middle", () => {
    const diff = computeDiff("abc", "aXc");
    expect(diff).toEqual({ utf16Start: 1, utf16RemovedLen: 1, inserted: "X" });
  });

  it("replaces the whole string when nothing is shared", () => {
    const diff = computeDiff("abc", "xyz");
    expect(diff).toEqual({ utf16Start: 0, utf16RemovedLen: 3, inserted: "xyz" });
  });

  it("inserts into an empty string", () => {
    const diff = computeDiff("", "hi");
    expect(diff).toEqual({ utf16Start: 0, utf16RemovedLen: 0, inserted: "hi" });
  });

  it("clears to an empty string", () => {
    const diff = computeDiff("hi", "");
    expect(diff).toEqual({ utf16Start: 0, utf16RemovedLen: 2, inserted: "" });
  });

  it("inserts an emoji as two UTF-16 code units", () => {
    // "a😀b": 😀 (U+1F600) is the surrogate pair D83D DE00 → 2 code units.
    const diff = computeDiff("ab", "a😀b");
    expect(diff).toEqual({ utf16Start: 1, utf16RemovedLen: 0, inserted: "😀" });
  });

  it("deletes a whole emoji (both code units)", () => {
    const diff = computeDiff("a😀b", "ab");
    expect(diff).toEqual({ utf16Start: 1, utf16RemovedLen: 2, inserted: "" });
  });

  it("inserts a CJK character (single BMP code unit)", () => {
    const diff = computeDiff("日本", "日X本");
    expect(diff).toEqual({ utf16Start: 1, utf16RemovedLen: 0, inserted: "X" });
  });

  it("appends a combining mark without resplitting the base char", () => {
    // "e" → "é" as base + combining acute (U+0301), not the precomposed é.
    const diff = computeDiff("e", "é");
    expect(diff).toEqual({ utf16Start: 1, utf16RemovedLen: 0, inserted: "́" });
  });

  it("does not split a surrogate pair when only the prefix high half matches", () => {
    // 😀 (D83D DE00) → 😺 (D83D DE3A): the high surrogate D83D is shared, but a
    // diff ending after it would split the pair. The boundary must clamp back to 0.
    const diff = computeDiff("😀", "😺");
    expect(diff).toEqual({ utf16Start: 0, utf16RemovedLen: 2, inserted: "😺" });
  });

  it("does not split a surrogate pair when only the suffix low half matches", () => {
    // 𠈀 (D840 DE00) → 😀 (D83D DE00): the low surrogate DE00 is shared as a
    // suffix, but starting the suffix there splits the pair. It must clamp.
    const diff = computeDiff("𠈀", "😀");
    expect(diff).toEqual({ utf16Start: 0, utf16RemovedLen: 2, inserted: "😀" });
  });
});
