import type { RubricPoint } from "@/api/generated/RubricPoint";

/** The core's rubric rules, checked in the browser so the editor can explain problems before saving. */
export function validateRubric(points: RubricPoint[], passThreshold: number): string[] {
  const errors: string[] = [];
  if (points.length === 0) errors.push("The rubric needs at least one point.");
  const seen = new Set<string>();
  for (const p of points) {
    const id = p.id.trim();
    if (id === "") {
      errors.push("Every point needs an id.");
      continue;
    }
    if (seen.has(id)) errors.push(`Point id "${id}" is used twice.`);
    seen.add(id);
    if (p.point.trim() === "") errors.push(`Point ${id} has no text.`);
    if (!(p.weight > 0)) errors.push(`Point ${id} needs a weight above 0.`);
  }
  if (!(passThreshold > 0 && passThreshold <= 1))
    errors.push("The pass threshold must be above 0% and at most 100%.");
  return errors;
}

/** The next free "r<n>" id. */
export function nextPointId(points: RubricPoint[]): string {
  const max = points.reduce((m, p) => Math.max(m, Number(/^r(\d+)$/.exec(p.id)?.[1] ?? 0)), 0);
  return `r${max + 1}`;
}
