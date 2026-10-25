import type { NodeKind } from "@/api/generated/NodeKind";
import type { TimelineNode } from "@/api/generated/TimelineNode";
import type { Measures } from "./colors";

/** Growing: sizes follow the selected day. Fixed: today's sizes throughout. */
export type SizeMode = "growing" | "fixed";

export type TreeNode = {
  id: string;
  kind: NodeKind;
  refId: number;
  label: string;
  createdOn: string;
  /** Questions counted toward the circle's area. */
  value: number;
  measures: Measures;
  children: TreeNode[];
};

const ROOT_ID = "root";

function measuresAt(n: TimelineNode, day: number | null): Measures {
  if (day === null) {
    const s = n.now;
    return {
      studied: s.studied,
      recall: s.recall,
      durability: s.durability,
      lastScore: s.last_score,
      daysSince: s.days_since,
    };
  }
  const s = n.series;
  return {
    studied: s.studied[day] ?? 0,
    recall: s.recall[day] ?? null,
    durability: s.durability[day] ?? null,
    lastScore: s.last_score[day] ?? null,
    daysSince: s.days_since[day] ?? null,
  };
}

/**
 * The tree under `rootId` (`null` = all top-level topics) as of `dayIndex`
 * (`null` = now). Returns `null` when `rootId` is not a topic in the timeline.
 */
export function buildTree(
  nodes: TimelineNode[],
  rootId: string | null,
  dayIndex: number | null,
  mode: SizeMode,
): TreeNode | null {
  const byParent = new Map<string | null, TimelineNode[]>();
  for (const n of nodes) {
    const list = byParent.get(n.parent);
    if (list) list.push(n);
    else byParent.set(n.parent, [n]);
  }
  const existsAt = (n: TimelineNode) =>
    dayIndex === null ? n.now.questions : (n.series.questions[dayIndex] ?? 0);
  const build = (n: TimelineNode): TreeNode | null => {
    const value = mode === "fixed" ? n.now.questions : existsAt(n);
    if (value === 0 && mode === "growing") return null;
    return {
      id: n.id,
      kind: n.kind,
      refId: n.ref_id,
      label: n.label,
      createdOn: n.created_on,
      value,
      measures: measuresAt(n, dayIndex),
      children: childrenOf(n.id),
    };
  };
  const childrenOf = (id: string | null) =>
    (byParent.get(id) ?? []).map(build).filter((c): c is TreeNode => c !== null);

  if (rootId === null) {
    const children = childrenOf(null);
    return {
      id: ROOT_ID,
      kind: "topic",
      refId: 0,
      label: "Topics",
      createdOn: "",
      value: children.reduce((sum, c) => sum + c.value, 0),
      measures: { studied: 0, recall: null, durability: null, lastScore: null, daysSince: null },
      children,
    };
  }
  const root = nodes.find((n) => n.id === rootId && n.kind === "topic");
  if (!root) return null;
  return {
    id: root.id,
    kind: root.kind,
    refId: root.ref_id,
    label: root.label,
    createdOn: root.created_on,
    value: existsAt(root),
    measures: measuresAt(root, dayIndex),
    children: childrenOf(root.id),
  };
}
