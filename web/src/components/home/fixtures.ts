import type { HomeSummary } from "@/api/generated/HomeSummary";

export const summary: HomeSummary = {
  today: {
    study_day: "2026-10-24",
    answered_today: 5,
    remaining_today: 0,
    new_introduced_today: 2,
    due_tomorrow: 3,
    streak: 1,
  },
  by_grade_today: [3, 1, 0, 1],
  best_streak: 4,
  new_waiting: 22,
  new_waiting_days: 8,
  pinned: [
    {
      pin_id: 1,
      question_id: 2,
      prompt: "Why are AC voltages quoted as RMS?",
      level: 1,
      topic_path: ["Electric circuits", "AC basics"],
      pinned_at: "2026-10-24T18:00:00Z",
    },
  ],
  leeches: [],
  ratings_unprocessed: 2,
};
