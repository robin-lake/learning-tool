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

/** Packs `node`'s children as leaves sized by their question counts, inside a `width`×`height` box. */
function packChildren(node: TreeNode, width: number, height: number, padding: number) {
  const h = hierarchy(node, (d) => (d === node ? d.children : null))
    .sum((d) => (d === node ? 0 : d.value))
    .sort((a, b) => order(a.data, b.data));
  return pack<TreeNode>().size([width, height]).padding(padding)(h).children ?? [];
}

/**
 * The root's children (depth 1) with areas proportional to their question counts, and each
 * one's children (depth 2) packed inside it. Packing the nested levels together would let
 * topics with many subtopics take more than their share of space.
 */
export const packLayout: Layout = (root, width, height) => {
  if (root.children.length === 0 || width <= 0 || height <= 0) return [];
  const shapes: Shape[] = [];
  for (const top of packChildren(root, width, height, 6)) {
    shapes.push({ id: top.data.id, x: top.x, y: top.y, r: top.r, depth: 1 });
    if (top.data.children.length === 0 || top.r < 8) continue;
    // Inset so inner rings don't touch their parent's edge.
    const r = top.r * 0.9;
    for (const inner of packChildren(top.data, 2 * r, 2 * r, 3)) {
      shapes.push({
        id: inner.data.id,
        x: top.x - r + inner.x,
        y: top.y - r + inner.y,
        r: inner.r,
        depth: 2,
      });
    }
  }
  return shapes;
};
