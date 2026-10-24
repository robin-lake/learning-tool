import { queryOptions, useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";
import type { CreateTopicBody } from "./generated/CreateTopicBody";
import type { DayActivity } from "./generated/DayActivity";
import type { HomeSummary } from "./generated/HomeSummary";
import type { Pin } from "./generated/Pin";
import type { Question } from "./generated/Question";
import type { QuestionPage } from "./generated/QuestionPage";
import type { QuestionPatch } from "./generated/QuestionPatch";
import type { QuestionStatus } from "./generated/QuestionStatus";
import type { Topic } from "./generated/Topic";
import type { TopicPage } from "./generated/TopicPage";
import type { UpdateTopicBody } from "./generated/UpdateTopicBody";

export const keys = {
  home: ["home"] as const,
  activity: (from: string, to: string) => ["activity", from, to] as const,
  topic: (id: number | null, all: boolean, archived: boolean) =>
    ["topic", id ?? "root", all, archived] as const,
  question: (id: number) => ["question", id] as const,
};

export const homeQuery = () =>
  queryOptions({ queryKey: keys.home, queryFn: () => api<HomeSummary>("GET", "/api/home") });

export const activityQuery = (from: string, to: string) =>
  queryOptions({
    queryKey: keys.activity(from, to),
    queryFn: () => api<DayActivity[]>("GET", `/api/activity?from=${from}&to=${to}`),
  });

export const topicQuery = (id: number | null, all = false, archived = false) =>
  queryOptions({
    queryKey: keys.topic(id, all, archived),
    queryFn: () =>
      api<TopicPage>(
        "GET",
        id === null
          ? `/api/topics?archived=${archived}`
          : `/api/topics/${id}?all=${all}&archived=${archived}`,
      ),
  });

export const questionQuery = (id: number) =>
  queryOptions({
    queryKey: keys.question(id),
    queryFn: () => api<QuestionPage>("GET", `/api/questions/${id}`),
  });

/** Edits can change counts anywhere (home tiles, topic cards, question rows), so refresh them all. */
function useRefreshAll() {
  const qc = useQueryClient();
  return () =>
    Promise.all([
      qc.invalidateQueries({ queryKey: keys.home }),
      qc.invalidateQueries({ queryKey: ["topic"] }),
      qc.invalidateQueries({ queryKey: ["question"] }),
    ]);
}

export function useCreateTopic() {
  const onSuccess = useRefreshAll();
  return useMutation({
    mutationFn: (body: CreateTopicBody) => api<Topic>("POST", "/api/topics", body),
    onSuccess,
  });
}

export function useUpdateTopic() {
  const onSuccess = useRefreshAll();
  return useMutation({
    mutationFn: ({ id, body }: { id: number; body: UpdateTopicBody }) =>
      api<Topic>("PATCH", `/api/topics/${id}`, body),
    onSuccess,
  });
}

export function useUpdateQuestion() {
  const onSuccess = useRefreshAll();
  return useMutation({
    mutationFn: ({ id, patch }: { id: number; patch: QuestionPatch }) =>
      api<Question>("PATCH", `/api/questions/${id}`, patch),
    onSuccess,
  });
}

export function useSetQuestionStatus() {
  const onSuccess = useRefreshAll();
  return useMutation({
    mutationFn: ({ id, status }: { id: number; status: QuestionStatus }) =>
      api<void>("POST", `/api/questions/${id}/status`, { status }),
    onSuccess,
  });
}

export function useRateQuestion() {
  const onSuccess = useRefreshAll();
  return useMutation({
    mutationFn: ({ id, usefulness, feedback }: { id: number; usefulness: number; feedback?: string }) =>
      api<{ id: number }>("POST", `/api/questions/${id}/ratings`, { usefulness, feedback }),
    onSuccess,
  });
}

export function usePin() {
  const onSuccess = useRefreshAll();
  return useMutation({ mutationFn: (id: number) => api<Pin>("PUT", `/api/questions/${id}/pin`), onSuccess });
}

export function useUnpin() {
  const onSuccess = useRefreshAll();
  return useMutation({
    mutationFn: (id: number) => api<void>("DELETE", `/api/questions/${id}/pin`),
    onSuccess,
  });
}
