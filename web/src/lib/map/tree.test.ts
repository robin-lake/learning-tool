import { describe, expect, it } from "vitest";
import { timeline } from "./fixtures";
import { buildTree } from "./tree";

describe("buildTree", () => {
  it("growing_drops_nonexistent", () => {
    const root = buildTree(timeline.nodes, null, 0, "growing");
    expect(root?.children.map((c) => c.id)).toEqual(["t1"]);
    expect(root?.children[0].value).toBe(1);
  });

  it("fixed_uses_now_sizes", () => {
    const root = buildTree(timeline.nodes, null, 0, "fixed");
    expect(root?.children.map((c) => [c.id, c.value])).toEqual([
      ["t1", 2],
      ["t2", 1],
    ]);
  });

  it("measures_from_day_index", () => {
    const day1 = buildTree(timeline.nodes, null, 1, "growing");
    const tools = day1?.children.find((c) => c.id === "t2");
    expect(tools?.measures.studied).toBe(0);
    const today = buildTree(timeline.nodes, null, null, "growing");
    expect(today?.children.find((c) => c.id === "t2")?.measures.studied).toBe(1);
  });

  it("zoom_root_children", () => {
    const zoomed = buildTree(timeline.nodes, "t1", null, "growing");
    expect(zoomed?.id).toBe("t1");
    expect(zoomed?.children.map((c) => [c.id, c.kind, c.value])).toEqual([
      ["q1", "question", 1],
      ["q2", "question", 1],
    ]);
  });

  it("unknown_root_returns_null", () => {
    expect(buildTree(timeline.nodes, "t99", null, "growing")).toBeNull();
  });
});

describe("fixed mode", () => {
  it("fixed_drops_nodes_absent_today", () => {
    const retired = timeline.nodes.map((n) =>
      n.id === "q3" ? { ...n, now: { ...n.now, questions: 0 } } : n,
    );
    const root = buildTree(retired, "t2", null, "fixed");
    expect(root?.children).toEqual([]);
  });
});
