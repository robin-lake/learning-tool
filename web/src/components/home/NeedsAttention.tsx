import { Link } from "@tanstack/react-router";
import type { HomeSummary } from "@/api/generated/HomeSummary";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

export function NeedsAttention({ summary }: { summary: HomeSummary }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">Needs attention</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        {summary.leeches.length === 0 ? (
          <p>No leeches: nothing is being failed over and over.</p>
        ) : (
          <div>
            <p className="font-medium">Leeches ({summary.leeches.length}): worth rewording</p>
            <ul className="mt-1 space-y-1">
              {summary.leeches.map((q) => (
                <li key={q.id} className="truncate" title={q.prompt}>
                  <Link
                    to="/questions/$questionId"
                    params={{ questionId: String(q.id) }}
                    className="underline"
                  >
                    {q.prompt}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
        )}
        <p>
          {summary.ratings_unprocessed === 0
            ? "No ratings waiting."
            : `${summary.ratings_unprocessed} rating${summary.ratings_unprocessed === 1 ? "" : "s"} waiting: run /refine-questions in Claude Code.`}
        </p>
      </CardContent>
    </Card>
  );
}
