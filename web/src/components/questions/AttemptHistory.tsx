import { format, parseISO } from "date-fns";
import type { ReviewRow } from "@/api/generated/ReviewRow";
import { Badge } from "@/components/ui/badge";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

const GRADE = { again: "Again", hard: "Hard", good: "Good", easy: "Easy" } as const;

/** Every answer to this question (and its earlier wordings), newest first; each expands to Claude's notes. */
export function AttemptHistory({ reviews, currentId }: { reviews: ReviewRow[]; currentId: number }) {
  if (reviews.length === 0) return <p className="text-sm text-muted-foreground">Not answered yet.</p>;
  const rows = [...reviews].sort((a, b) => b.reviewed_at.localeCompare(a.reviewed_at));
  return (
    <Table className="table-fixed">
      <TableHeader>
        <TableRow>
          <TableHead className="w-32">When</TableHead>
          <TableHead className="w-1/4">Asked as</TableHead>
          <TableHead>Your answer</TableHead>
          <TableHead className="w-28">Points</TableHead>
          <TableHead className="w-16">Score</TableHead>
          <TableHead className="w-20">Grade</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((r) => (
          <TableRow key={r.id} className="align-top">
            <TableCell className="whitespace-normal">
              {format(parseISO(r.reviewed_at), "d MMM, HH:mm")}
              {r.question_id !== currentId && (
                <div>
                  <Badge variant="secondary">earlier wording</Badge>
                </div>
              )}
            </TableCell>
            <TableCell className="truncate" title={r.asked_prompt}>
              {r.asked_prompt}
            </TableCell>
            <TableCell className="whitespace-normal">
              <details>
                <summary className="cursor-pointer truncate" title={r.user_answer}>
                  {r.user_answer}
                </summary>
                <div className="mt-2 space-y-1 text-sm">
                  <p className="whitespace-pre-wrap">{r.user_answer}</p>
                  <ul className="text-muted-foreground">
                    {r.rubric_result.map((p) => (
                      <li key={p.id}>
                        {p.hit ? "✓" : "✗"} {p.id}
                        {p.note && `: ${p.note}`}
                      </li>
                    ))}
                  </ul>
                  {r.feedback && <p>Feedback: {r.feedback}</p>}
                  {r.hints_used > 0 && <p>Hints used: {r.hints_used}</p>}
                </div>
              </details>
            </TableCell>
            <TableCell className="whitespace-normal">
              {r.rubric_result.map((p) => (
                <span key={p.id} className={p.hit ? "mr-2" : "mr-2 text-destructive"}>
                  {p.hit ? "✓" : "✗"} {p.id}
                </span>
              ))}
            </TableCell>
            <TableCell>{Math.round(r.score * 100)}%</TableCell>
            <TableCell>
              <Badge variant={r.grade === "again" ? "destructive" : "outline"}>{GRADE[r.grade]}</Badge>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
