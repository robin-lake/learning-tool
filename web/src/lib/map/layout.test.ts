import { describe, expect, it } from "vitest";
import { timeline } from "./fixtures";
import { packLayout } from "./layout";
import { buildTree } from "./tree";

describe("packLayout", () => {
  it("stable_order_between_days", () => {
    const a = buildTree(timeline.nodes, null, 1, "growing");
    const b = buildTree(timeline.nodes, null, 2, "growing");
    if (!a || !b) throw new Error("no tree");
    const ids = (root: typeof a) =>
      packLayout(root, 400, 300)
        .filter((s) => s.depth === 1)
        .map((s) => s.id);
    expect(ids(a)).toEqual(["t1", "t2"]);
    expect(ids(b)).toEqual(["t1", "t2"]);
  });

  it("circles_inside_bounds", () => {
    const root = buildTree(timeline.nodes, null, null, "fixed");
    if (!root) throw new Error("no tree");
    for (const s of packLayout(root, 400, 300)) {
      expect(s.x - s.r).toBeGreaterThanOrEqual(-1e-6);
      expect(s.x + s.r).toBeLessThanOrEqual(400 + 1e-6);
      expect(s.y - s.r).toBeGreaterThanOrEqual(-1e-6);
      expect(s.y + s.r).toBeLessThanOrEqual(300 + 1e-6);
      expect(s.r).toBeGreaterThan(0);
    }
  });
});
