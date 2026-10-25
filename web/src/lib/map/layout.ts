import { hierarchy, pack } from "d3-hierarchy";
import type { TreeNode } from "./tree";

/** A circle (or, for another layout, the circle enclosing its shape) for one of the root's children. */
export type Shape = { id: string; x: number; y: number; r: number };

/** Swappable: a treemap layout would produce the same shapes from the same tree. */
export type Layout = (root: TreeNode, width: number, height: number) => Shape[];

/** Creation order keeps circles in place as sizes change between days. */
function order(a: TreeNode, b: TreeNode): number {
  if (a.createdOn !== b.createdOn) return a.createdOn < b.createdOn ? -1 : 1;
  return a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
}

/** Packs `node`'s children as leaves sized by their question counts, inside a `width`×`height` box. */
function packChildren(node: TreeNode, width: number, height: number, padding: number) {
  const h = hierarchy(node, (d) => (d === node ? d.children : null))
    .sum((d) => (d === node ? 0 : d.value))
    .sort((a, b) => order(a.data, b.data));
  return pack<TreeNode>().size([width, height]).padding(padding)(h).children ?? [];
}

/** The root's children, with areas proportional to their question counts. */
export const packLayout: Layout = (root, width, height) => {
  // d3 pack yields NaN when every value is zero.
  if (root.children.every((c) => c.value <= 0) || width <= 0 || height <= 0) return [];
  return packChildren(root, width, height, 6).map((c) => ({ id: c.data.id, x: c.x, y: c.y, r: c.r }));
};
