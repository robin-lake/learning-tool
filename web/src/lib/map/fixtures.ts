import type { Snapshot } from "@/api/generated/Snapshot";
import type { Timeline } from "@/api/generated/Timeline";
import type { TimelineNode } from "@/api/generated/TimelineNode";

const snap = (questions: number, studied: number, recall: number | null): Snapshot => ({
  questions,
  studied,
  recall,
  durability: studied ? 5 : null,
  last_score: studied ? 0.7 : null,
  days_since: studied ? 1 : null,
  due: 0,
});

function node(
  id: string,
  parent: string | null,
  created_on: string,
  questions: number[],
  studied: number[],
  label = id,
): TimelineNode {
  const kind = id.startsWith("t") ? "topic" : "question";
  return {
    id,
    kind,
    ref_id: Number(id.slice(1)),
    parent,
    label,
    target_level: kind === "topic" ? 1 : null,
    created_on,
    series: {
      questions,
      studied,
      recall: studied.map((s) => (s ? 0.8 : null)),
      durability: studied.map((s) => (s ? 5 : null)),
      last_score: studied.map((s) => (s ? 0.7 : null)),
      days_since: studied.map((s) => (s ? 0 : null)),
      due: questions.map(() => 0),
    },
    now: snap(
      questions[questions.length - 1],
      studied[studied.length - 1],
      studied[studied.length - 1] ? 0.8 : null,
    ),
  };
}

/** Circuits (q1 from day 0, q2 from day 2) and Tools (q3 from day 1); 3 days of history. */
export const timeline: Timeline = {
  days: ["2026-10-01", "2026-10-02", "2026-10-03"],
  today: "2026-10-03",
  stale_after_days: 14,
  stale_full_days: 60,
  nodes: [
    node("t1", null, "2026-10-01", [1, 1, 2], [1, 1, 1], "Electric circuits"),
    node("t2", null, "2026-10-02", [0, 1, 1], [0, 0, 1], "Speaker design tools"),
    node("q1", "t1", "2026-10-01", [1, 1, 1], [1, 1, 1]),
    node("q2", "t1", "2026-10-03", [0, 0, 1], [0, 0, 0]),
    node("q3", "t2", "2026-10-02", [0, 1, 1], [0, 0, 1]),
  ],
};
