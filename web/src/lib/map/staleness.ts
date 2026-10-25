/**
 * How fresh a measure still is: 1 up to `after` days since the last review,
 * falling linearly to 0 at `full` days. Never-studied (`null`) counts as fresh;
 * callers show it separately.
 */
export function freshness(daysSince: number | null, after: number, full: number): number {
  if (daysSince === null || daysSince <= after) return 1;
  return Math.max(0, Math.min(1, (full - daysSince) / (full - after)));
}
