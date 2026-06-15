import { describe, it, expect } from "vitest";
import { groupTokensByKind } from "./highlights";

describe("groupTokensByKind", () => {
  it("groups flat token triples by kind", () => {
    const flat = new Uint32Array([0, 2, 0, 2, 9, 1]);
    expect(groupTokensByKind(flat)).toEqual({
      0: [[0, 2]],
      1: [[2, 9]],
    });
  });

  it("collects multiple ranges of the same kind in order", () => {
    // Text, Variable, Text, Variable
    const flat = new Uint32Array([0, 2, 0, 2, 9, 1, 9, 11, 0, 11, 18, 1]);
    expect(groupTokensByKind(flat)).toEqual({
      0: [
        [0, 2],
        [9, 11],
      ],
      1: [
        [2, 9],
        [11, 18],
      ],
    });
  });

  it("returns an empty record for an empty token array", () => {
    expect(groupTokensByKind(new Uint32Array([]))).toEqual({});
  });
});
