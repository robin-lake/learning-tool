import { Link } from "@tanstack/react-router";
import { format, parseISO } from "date-fns";
import { Pin } from "lucide-react";
import type { QuestionRow } from "@/api/generated/QuestionRow";
import { usePin, useUnpin } from "@/api/queries";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { cn } from "@/lib/utils";

function PinToggle({ row }: { row: QuestionRow }) {
  const pin = usePin();
  const unpin = useUnpin();
  const busy = pin.isPending || unpin.isPending;
  return (
    <Button
      variant="ghost"
      size="sm"
      aria-pressed={row.pinned}
      aria-label={row.pinned ? "Unpin" : "Pin for next session"}
      title={row.pinned ? "Pinned for your next session (click to unpin)" : "Pin for your next session"}
      disabled={busy}
      onClick={() => (row.pinned ? unpin.mutate(row.id) : pin.mutate(row.id))}
    >
      <Pin className={cn("size-4", row.pinned ? "fill-current" : "opacity-40")} />
    </Button>
  );
}

export function QuestionTable({ rows }: { rows: QuestionRow[] }) {
  return (
    <Table className="table-fixed">
      <TableHeader>
        <TableRow>
          <TableHead className="w-14">Level</TableHead>
          <TableHead>Question</TableHead>
          <TableHead className="w-24">Status</TableHead>
          <TableHead className="w-28">Next due</TableHead>
          <TableHead className="w-14 text-right">Pin</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {rows.map((r) => (
          <TableRow key={r.id}>
            <TableCell>L{r.level}</TableCell>
            <TableCell>
              <Link
                to="/questions/$questionId"
                params={{ questionId: String(r.id) }}
                title={r.prompt}
                className="block truncate underline-offset-2 hover:underline"
              >
                {r.prompt}
              </Link>
            </TableCell>
            <TableCell>
              <Badge variant={r.status === "active" ? "outline" : "secondary"}>
                {r.reviews === 0 && r.status === "active" ? "new" : r.status}
              </Badge>
            </TableCell>
            <TableCell className="text-muted-foreground">
              {r.due_on ? format(parseISO(r.due_on), "d MMM") : "—"}
            </TableCell>
            <TableCell className="text-right">
              <PinToggle row={r} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
