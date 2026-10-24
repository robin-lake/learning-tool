import { format, parseISO } from "date-fns";
import type { QuestionPage } from "@/api/generated/QuestionPage";

export function ScheduleBox({ page }: { page: QuestionPage }) {
  const s = page.detail.state;
  return (
    <div className="space-y-1 text-sm">
      <div className="text-xs uppercase tracking-wide text-muted-foreground">Schedule</div>
      {s.reps === 0 ? (
        <p>Not studied yet.</p>
      ) : (
        <>
          <p>Due: {s.due_on ? format(parseISO(s.due_on), "EEE d MMM") : "—"}</p>
          <p>
            Reviews {s.reps} · lapses {s.lapses}
          </p>
          {page.recall_now !== null && <p>Recall now: {Math.round(page.recall_now * 100)}%</p>}
        </>
      )}
    </div>
  );
}
