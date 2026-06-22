import { describe, it, expect } from "vitest";
import { CachingMeasurePort, resolveLineHeight } from "./measure.js";
import type { FontMetrics } from "./measure.js";

describe("CachingMeasurePort", () => {
  it("measures once and caches the result across calls (ADR-0004)", () => {
    let calls = 0;
    const port = new CachingMeasurePort(() => {
      calls += 1;
      return { advance: 8, lineHeight: 21 };
    });

    const a = port.metrics();
    const b = port.metrics();

    expect(calls).toBe(1);
    expect(a).toEqual({ advance: 8, lineHeight: 21 });
    expect(b).toBe(a); // same cached object
  });

  it("returns whatever the injected measurer produced", () => {
    const expected: FontMetrics = { advance: 9.6, lineHeight: 24 };
    const port = new CachingMeasurePort(() => expected);
    expect(port.metrics()).toBe(expected);
  });
});

describe("resolveLineHeight", () => {
  it("uses a px computed line-height directly", () => {
    expect(resolveLineHeight({ lineHeight: "21px", fontSize: "14px" })).toBe(21);
  });

  it("approximates a 'normal' line-height from the font size (~1.2)", () => {
    expect(resolveLineHeight({ lineHeight: "normal", fontSize: "10px" })).toBeCloseTo(12);
  });

  it("falls back to 16px font size when font size is unparseable", () => {
    expect(resolveLineHeight({ lineHeight: "normal", fontSize: "" })).toBeCloseTo(19.2);
  });
});
