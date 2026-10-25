import { format, parseISO } from "date-fns";
import type { QuestionPage } from "@/api/generated/QuestionPage";

function Row({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-right tabular-nums">{value}</dd>
    </>
  );
}

export function ScheduleBox({ page }: { page: QuestionPage }) {
  const s = page.detail.state;
  return (
    <section className="space-y-2">
      <h2 className="font-semibold">Schedule</h2>
      {s.reps === 0 ? (
        <p className="text-sm text-muted-foreground">Not studied yet.</p>
      ) : (
        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-sm">
          <Row label="Due" value={s.due_on ? format(parseISO(s.due_on), "EEE d MMM") : "—"} />
          {page.recall_now !== null && (
            <Row label="Recall now" value={`${Math.round(page.recall_now * 100)}%`} />
          )}
          <Row label="Reviews" value={String(s.reps)} />
          <Row label="Lapses" value={String(s.lapses)} />
        </dl>
      )}
    </section>
  );
}
