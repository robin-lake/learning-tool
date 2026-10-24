import { addDays, differenceInCalendarDays, format, parseISO, startOfWeek, subWeeks } from "date-fns";
import type { DayActivity } from "@/api/generated/DayActivity";

export type Level = 0 | 1 | 2 | 3 | 4;

export type HeatCell = {
  date: string;
  count: number;
  /** Again, hard, good, easy (the API's order). */
  byGrade: [number, number, number, number];
  topics: string[];
  level: Level;
  isToday: boolean;
};

const WEEKS = 53;

export function levelFor(count: number): Level {
  if (count === 0) return 0;
  if (count <= 2) return 1;
  if (count <= 5) return 2;
  if (count <= 10) return 3;
  return 4;
}

/** "Easy · Good · Hard · Again", in the order people read grades (best first). */
export function gradeLine([again, hard, good, easy]: [number, number, number, number]): string {
  return `Easy ${easy} · Good ${good} · Hard ${hard} · Again ${again}`;
}

/** 53 Sunday-first week columns ending with the week containing `today`; days after today are left out. */
export function buildHeatmap(activity: DayActivity[], today: string): HeatCell[][] {
  const end = parseISO(today);
  const start = subWeeks(startOfWeek(end), WEEKS - 1);
  const byDay = new Map(activity.map((a) => [a.study_day, a]));
  const weeks: HeatCell[][] = [];
  for (let d = start; d <= end; d = addDays(d, 1)) {
    const date = format(d, "yyyy-MM-dd");
    const a = byDay.get(date);
    const count = a?.count ?? 0;
    const week = Math.floor(differenceInCalendarDays(d, start) / 7);
    weeks[week] ??= [];
    weeks[week].push({
      date,
      count,
      byGrade: a?.by_grade ?? [0, 0, 0, 0],
      topics: a?.topics.map((t) => t.name) ?? [],
      level: levelFor(count),
      isToday: date === today,
    });
  }
  return weeks;
}
