import { describe, expect, it } from "vitest";
import { timeline } from "./fixtures";
import { packLayout } from "./layout";
import { buildTree, type TreeNode } from "./tree";

describe("packLayout", () => {
  it("stable_order_between_days", () => {
    const a = buildTree(timeline.nodes, null, 1, "growing");
    const b = buildTree(timeline.nodes, null, 2, "growing");
    if (!a || !b) throw new Error("no tree");
    const ids = (root: typeof a) => packLayout(root, 400, 300).map((s) => s.id);
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

it("top_level_areas_follow_question_counts", () => {
  const leaf = (id: string, value: number, children: TreeNode[] = []): TreeNode => ({
    id,
    kind: children.length || id.startsWith("t") ? "topic" : "question",
    refId: 0,
    label: id,
    createdOn: "2026-10-01",
    value,
    due: 0,
    targetLevel: null,
    measures: { studied: 0, recall: null, durability: null, lastScore: null, daysSince: null },
    children,
  });
  const questions = (prefix: string, n: number) =>
    Array.from({ length: n }, (_, i) => leaf(`q${prefix}${i}`, 1));
  // 17 questions spread over four subtopics vs 10 in one subtopic, like the real data.
  const big = leaf("t1", 17, [
    leaf("t11", 5, questions("a", 5)),
    leaf("t12", 4, questions("b", 4)),
    leaf("t13", 5, questions("c", 5)),
    leaf("t14", 3, questions("d", 3)),
  ]);
  const small = leaf("t2", 10, [leaf("t21", 10, questions("e", 10))]);
  const shapes = packLayout(leaf("root", 27, [big, small]), 455, 360);
  const r = (id: string) => shapes.find((s) => s.id === id)?.r ?? 0;
  const areaRatio = (r("t1") / r("t2")) ** 2;
  expect(areaRatio).toBeGreaterThan(1.7 * 0.75);
  expect(areaRatio).toBeLessThan(1.7 * 1.25);
});

it("all_zero_children_give_no_shapes", () => {
  const root = buildTree(timeline.nodes, null, null, "fixed");
  if (!root) throw new Error("no tree");
  const zeroed = { ...root, children: root.children.map((c) => ({ ...c, value: 0, children: [] })) };
  expect(packLayout(zeroed, 400, 300)).toEqual([]);
});

it("only_the_root_children_get_shapes", () => {
  const root = buildTree(timeline.nodes, null, null, "fixed");
  if (!root) throw new Error("no tree");
  expect(packLayout(root, 400, 300).map((s) => s.id)).toEqual(["t1", "t2"]);
});
