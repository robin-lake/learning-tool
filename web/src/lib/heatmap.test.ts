import { describe, expect, it } from "vitest";
import type { DayActivity } from "@/api/generated/DayActivity";
import { buildHeatmap, levelFor } from "./heatmap";

const day = (study_day: string, count: number): DayActivity => ({
  study_day,
  count,
  by_grade: [0, 0, count, 0],
  topics: [{ id: 1, name: "Impedance" }],
});

describe("buildHeatmap", () => {
  it("buildHeatmap_has_53_weeks_of_7", () => {
    const weeks = buildHeatmap([], "2026-10-24"); // a Saturday: the last week is complete
    expect(weeks).toHaveLength(53);
    expect(weeks.every((w) => w.length === 7)).toBe(true);
    expect(weeks[0][0].date).toBe("2025-10-19"); // the Sunday 52 weeks before this week
  });

  it("levels_follow_thresholds", () => {
    expect([0, 1, 2, 3, 5, 6, 10, 11, 40].map(levelFor)).toEqual([0, 1, 1, 2, 2, 3, 3, 4, 4]);
  });

  it("today_is_last_week_and_flagged", () => {
    const weeks = buildHeatmap([], "2026-10-21"); // a Wednesday
    const last = weeks[weeks.length - 1];
    expect(last.map((c) => c.date)).toEqual(["2026-10-18", "2026-10-19", "2026-10-20", "2026-10-21"]);
    expect(last[3].isToday).toBe(true);
    expect(weeks.flat().filter((c) => c.isToday)).toHaveLength(1);
  });

  it("days_without_activity_are_zero", () => {
    const weeks = buildHeatmap([day("2026-10-20", 6)], "2026-10-24");
    const cells = weeks.flat();
    const active = cells.find((c) => c.date === "2026-10-20");
    expect(active).toMatchObject({ count: 6, level: 3, topics: ["Impedance"], byGrade: [0, 0, 6, 0] });
    expect(cells.filter((c) => c.count > 0)).toHaveLength(1);
  });
});
