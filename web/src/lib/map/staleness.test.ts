import { describe, expect, it } from "vitest";
import { freshness } from "./staleness";

describe("freshness", () => {
  it("fresh_until_after", () => {
    expect(freshness(0, 14, 60)).toBe(1);
    expect(freshness(14, 14, 60)).toBe(1);
    expect(freshness(null, 14, 60)).toBe(1);
  });
  it("linear_to_full", () => {
    expect(freshness(37, 14, 60)).toBeCloseTo(0.5, 5);
  });
  it("clamped", () => {
    expect(freshness(60, 14, 60)).toBe(0);
    expect(freshness(400, 14, 60)).toBe(0);
  });
});
