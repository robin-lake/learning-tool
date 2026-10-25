import { hierarchy, pack } from "d3-hierarchy";
import type { TreeNode } from "./tree";

/** A circle (or, for another layout, the circle enclosing its shape). Depth 1 = the root's children. */
export type Shape = { id: string; x: number; y: number; r: number; depth: number };

/** Swappable: a treemap layout would produce the same shapes from the same tree. */
export type Layout = (root: TreeNode, width: number, height: number) => Shape[];

/** Creation order keeps circles in place as sizes change between days. */
function order(a: TreeNode, b: TreeNode): number {
  if (a.createdOn !== b.createdOn) return a.createdOn < b.createdOn ? -1 : 1;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

export const packLayout: Layout = (root, width, height) => {
  if (root.children.length === 0 || width <= 0 || height <= 0) return [];
  const h = hierarchy(root)
    .sum((d) => (d.children.length ? 0 : d.value))
    .sort((a, b) => order(a.data, b.data));
  const packed = pack<TreeNode>().size([width, height]).padding(6)(h);
  return packed
    .descendants()
    .filter((d) => d.depth > 0)
    .map((d) => ({ id: d.data.id, x: d.x, y: d.y, r: d.r, depth: d.depth }));
};
