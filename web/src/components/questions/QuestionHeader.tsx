import { Link } from "@tanstack/react-router";
import { Pin } from "lucide-react";
import type { QuestionPage } from "@/api/generated/QuestionPage";
import { usePin, useSetQuestionStatus, useUnpin } from "@/api/queries";
import { Breadcrumbs } from "@/components/topics/Breadcrumbs";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { EditQuestionDialog } from "./EditQuestionDialog";

const LEVELS = ["Orientation", "Working knowledge", "Applied", "Expert"];

export function QuestionHeader({ page }: { page: QuestionPage }) {
  const q = page.detail.question;
  const pin = usePin();
  const unpin = useUnpin();
  const status = useSetQuestionStatus();
  const retired = q.status === "retired";
  return (
    <div className="space-y-4">
      <Breadcrumbs path={[...page.topic_refs, { id: -q.id, name: `Question ${q.id}` }]} />
      {retired && (
        <Alert>
          <AlertDescription>
            This wording was replaced.{" "}
            {page.newest_revision_id !== null && (
              <Link
                to="/questions/$questionId"
                params={{ questionId: String(page.newest_revision_id) }}
                className="underline"
              >
                See the newest version
              </Link>
            )}
          </AlertDescription>
        </Alert>
      )}
      <div className="space-y-3">
        <div className="flex flex-wrap items-center gap-2 text-sm text-muted-foreground">
          <Badge variant="outline">L{q.level}</Badge>
          <span>{LEVELS[q.level] ?? ""}</span>
          {q.status !== "active" && <Badge variant="secondary">{q.status}</Badge>}
        </div>
        <h1 className="max-w-[42rem] text-3xl font-bold leading-tight tracking-tight text-balance">
          {q.prompt}
        </h1>
        {!retired && (
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              aria-pressed={page.pinned}
              disabled={pin.isPending || unpin.isPending || q.status !== "active"}
              onClick={() => (page.pinned ? unpin.mutate(q.id) : pin.mutate(q.id))}
              title="Pinned questions come first in your next study session"
            >
              <Pin className={cn("size-4", page.pinned && "fill-current")} />
              {page.pinned ? "Pinned" : "Pin"}
            </Button>
            <EditQuestionDialog question={q} />
            <Button
              variant="outline"
              size="sm"
              disabled={status.isPending}
              onClick={() =>
                status.mutate({ id: q.id, status: q.status === "suspended" ? "active" : "suspended" })
              }
            >
              {q.status === "suspended" ? "Unsuspend" : "Suspend"}
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
