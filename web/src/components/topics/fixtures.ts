import type { QuestionRow } from "@/api/generated/QuestionRow";
import type { Topic } from "@/api/generated/Topic";
import type { TopicCard } from "@/api/generated/TopicCard";

export const topic: Topic = {
  id: 4,
  parent_id: 1,
  name: "Impedance",
  slug: "impedance",
  description: "",
  scope: "main",
  project_id: null,
  target_level: 1,
  status: "active",
  position: 2,
};

export const card: TopicCard = {
  id: 5,
  name: "Loudspeaker impedance",
  target_level: 1,
  status: "active",
  questions_by_level: [1, 4, 0, 0],
  active_questions: 4,
  studied: 3,
  avg_retrievability: 0.82,
};

export const row = (over: Partial<QuestionRow> = {}): QuestionRow => ({
  id: 1,
  topic_id: 4,
  level: 1,
  prompt: "How is impedance written as a complex number?",
  status: "active",
  due_on: null,
  pinned: false,
  recall_now: null,
  reviews: 0,
  ...over,
});
