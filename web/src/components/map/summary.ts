import type { TreeNode } from "@/lib/map/tree";

export const pct = (x: number) => `${Math.round(x * 100)}%`;
export const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** The accessible name of a circle: its full label and a short summary. */
export function summary(n: TreeNode): string {
  const studied = n.measures.studied > 0 && n.measures.recall !== null;
  return `${n.label}: ${plural(n.value, "question")}, ${studied ? `recall ${pct(n.measures.recall ?? 0)}` : "not studied"}`;
}
