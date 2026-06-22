import { describe, it, expect } from "vitest";
import { instanceHighlights } from "./highlight-names.js";

describe("instanceHighlights", () => {
  it("maps each painted HighlightKind to an instance-scoped registry name", () => {
    const { nameByKind } = instanceHighlights("7");
    expect(nameByKind).toEqual({
      1: "vellum-7-variable",
      2: "vellum-7-keyword",
      3: "vellum-7-comment",
    });
  });

  it("does not name kind 0 (Text is painted by the surface, not a highlight)", () => {
    const { nameByKind } = instanceHighlights("0");
    expect(nameByKind[0]).toBeUndefined();
  });

  it("gives two instances fully disjoint names so they cannot clobber each other", () => {
    const a = instanceHighlights("0").nameByKind;
    const b = instanceHighlights("1").nameByKind;
    const shared = Object.values(a).filter((name) => Object.values(b).includes(name));
    expect(shared).toEqual([]);
  });

  it("emits a ::highlight() style rule for every named kind", () => {
    const { nameByKind, styleText } = instanceHighlights("3");
    for (const name of Object.values(nameByKind)) {
      expect(styleText).toContain(`::highlight(${name})`);
    }
  });

  it("styles each kind with its color, comment also italic (this is the sole color source)", () => {
    const { styleText } = instanceHighlights("4");
    expect(styleText).toContain("color: #7c5cff;"); // variable
    expect(styleText).toContain("color: #d08770;"); // keyword
    expect(styleText).toContain("color: #8a8f98;"); // comment
    expect(styleText).toContain("font-style: italic");
  });

  it("gives an instance-scoped selection highlight with a background rule", () => {
    const { selectionName, styleText } = instanceHighlights("5");
    expect(selectionName).toBe("vellum-5-selection");
    expect(styleText).toContain("::highlight(vellum-5-selection)");
    expect(styleText).toContain("background-color");
  });
});
