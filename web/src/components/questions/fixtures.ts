import type { QuestionPage } from "@/api/generated/QuestionPage";
import type { ReviewRow } from "@/api/generated/ReviewRow";

export const review = (over: Partial<ReviewRow> = {}): ReviewRow => ({
  id: 1,
  question_id: 2,
  session_id: 1,
  reviewed_at: "2026-10-24T18:00:00Z",
  grade: "again",
  score: 0.5,
  rubric_result: [
    { id: "r1", hit: true, note: "Purpose right" },
    { id: "r2", hit: false, note: "RMS is 0.707 x peak, not half" },
  ],
  hints_used: 0,
  fluent: false,
  user_answer: "It gives the average load. RMS is about half the peak.",
  asked_prompt: "Why are AC voltages quoted as RMS?",
  feedback: "Purpose right; RMS = peak / sqrt(2).",
  duration_ms: null,
  ...over,
});

export const page = (over: Partial<QuestionPage> = {}): QuestionPage => ({
  detail: {
    question: {
      id: 2,
      topic_id: 2,
      level: 1,
      prompt: "Why are AC voltages quoted as RMS, and how does RMS relate to peak?",
      answer: "RMS is the DC-equivalent value giving the same average power; RMS = peak / sqrt(2).",
      rubric: [
        { id: "r1", point: "DC-equivalent value giving the same average power", weight: 1, essential: true },
        { id: "r2", point: "RMS = peak / sqrt(2)", weight: 1, essential: false },
      ],
      pass_threshold: 0.6,
      source_id: 2,
      source_locator: "Capacitor (part 1)",
      scope: "main",
      project_id: null,
      status: "active",
      priority: 1,
      guide_version_id: 1,
      revision_of: null,
      created_at: "2026-10-24T10:00:00Z",
      updated_at: "2026-10-24T10:00:00Z",
    },
    topic_path: ["Electric circuits", "AC basics and resistance"],
    secondary_topic_ids: [],
    state: {
      question_id: 2,
      mem: { stability: 1.2, difficulty: 6 },
      due_on: "2026-10-25",
      last_reviewed_at: "2026-10-24T18:00:00Z",
      last_study_day: "2026-10-24",
      reps: 2,
      lapses: 1,
      state: "relearning",
    },
    reviews: [review(), review({ id: 2, reviewed_at: "2026-10-24T18:20:00Z", score: 0, grade: "again" })],
    ratings: [],
    revisions: [],
  },
  topic_refs: [
    { id: 1, name: "Electric circuits" },
    { id: 2, name: "AC basics and resistance" },
  ],
  pinned: false,
  recall_now: 0.72,
  source_citation: "University Physics Vol. 2, 15.2 Simple AC Circuits, Capacitor (part 1)",
  newest_revision_id: null,
  ...over,
});
