import { describe, expect, it } from "vitest";
import { BANDS, colorFor, hueFor, type Measures, parseOklch } from "./colors";

const stale = { after: 14, full: 60 };
const m = (over: Partial<Measures> = {}): Measures => ({
  studied: 1,
  recall: 0.8,
  durability: 10,
  lastScore: 0.8,
  daysSince: 0,
  ...over,
});

describe("colors", () => {
  it("never_studied_is_hatched", () => {
    const c = colorFor(
      m({ studied: 0, recall: null, durability: null, lastScore: null, daysSince: null }),
      "combined",
      stale,
    );
    expect(c.hatched).toBe(true);
  });

  it("combined_respects_bands", () => {
    for (const lastScore of [0, 0.3, 0.7, 1]) {
      for (const durability of [0.1, 1, 10, 90, 400]) {
        for (const daysSince of [0, 20, 60, 500]) {
          const { l, c } = parseOklch(
            colorFor(m({ lastScore, durability, daysSince }), "combined", stale).fill,
          );
          expect(l).toBeGreaterThanOrEqual(BANDS.lightDurable - 1e-9);
          expect(l).toBeLessThanOrEqual(BANDS.lightFragile + 1e-9);
          expect(c).toBeGreaterThanOrEqual(BANDS.chromaFloor - 1e-9);
        }
      }
    }
  });

  it("hue_goes_through_purple_not_green", () => {
    expect(hueFor(0)).toBeCloseTo(25, 5);
    expect(hueFor(1)).toBeCloseTo(255, 5);
    expect(Math.abs(hueFor(0.5) - 320)).toBeLessThan(5);
    for (let t = 0; t <= 1; t += 0.05) {
      const h = hueFor(t);
      expect(h > 90 && h < 200).toBe(false);
    }
  });

  it("stale_lowers_chroma_to_floor_not_zero", () => {
    const fresh = parseOklch(colorFor(m({ daysSince: 0 }), "combined", stale).fill).c;
    const old = parseOklch(colorFor(m({ daysSince: 500 }), "combined", stale).fill).c;
    expect(fresh).toBeCloseTo(BANDS.chromaFresh, 5);
    expect(old).toBeCloseTo(BANDS.chromaFloor, 5);
  });

  it("text_contrast_switches", () => {
    expect(colorFor(m({ durability: 90 }), "combined", stale).text).toBe("light");
    expect(colorFor(m({ durability: 1 }), "combined", stale).text).toBe("dark");
  });

  it("single_measure_views_fixed_light_and_chroma", () => {
    for (const mode of ["recall", "durability", "score", "staleness"] as const) {
      const { l, c } = parseOklch(colorFor(m(), mode, stale).fill);
      expect(l).toBeCloseTo(0.65, 5);
      expect(c).toBeCloseTo(0.13, 5);
    }
    const weak = parseOklch(colorFor(m({ recall: 0.1 }), "recall", stale).fill).h;
    const strong = parseOklch(colorFor(m({ recall: 0.95 }), "recall", stale).fill).h;
    expect(Math.abs(weak - hueFor(0.1))).toBeLessThan(1e-6);
    expect(Math.abs(strong - hueFor(0.95))).toBeLessThan(1e-6);
    const staleView = parseOklch(colorFor(m({ daysSince: 500 }), "staleness", stale).fill).h;
    expect(staleView).toBeCloseTo(hueFor(0), 5);
  });
});
