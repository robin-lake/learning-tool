import { Link } from "@tanstack/react-router";
import type { HomeSummary } from "@/api/generated/HomeSummary";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function ComingUp({ summary }: { summary: HomeSummary }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Coming up</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        <p>Tomorrow: {summary.today.due_tomorrow} reviews due, plus new questions as the daily cap allows.</p>
        <p>
          {summary.new_waiting} new questions waiting (about {summary.new_waiting_days} days at the current
          cap).
        </p>
        <div>
          <p className="font-medium">Pinned for your next session ({summary.pinned.length})</p>
          {summary.pinned.length === 0 ? (
            <p className="text-muted-foreground">Nothing pinned. Pin questions from their pages.</p>
          ) : (
            <ul className="mt-1 space-y-1">
              {summary.pinned.map((p) => (
                <li key={p.pin_id} className="truncate" title={p.prompt}>
                  <Link
                    to="/questions/$questionId"
                    params={{ questionId: String(p.question_id) }}
                    className="underline"
                  >
                    {p.prompt}
                  </Link>
                  <span className="text-muted-foreground"> · {p.topic_path.join(" › ")}</span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
