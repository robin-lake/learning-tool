import { useQuery } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { topicQuery } from "@/api/queries";
import { ErrorState } from "@/components/ErrorState";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { Breadcrumbs } from "./Breadcrumbs";
import { QuestionTable } from "./QuestionTable";
import { SubtopicCards } from "./SubtopicCards";
import { ArchiveTopicDialog, NewTopicDialog, RenameTopicDialog } from "./TopicDialogs";

/** URL search params; each is present only when switched on, so links don't need to set them. */
export type TopicSearch = { all?: boolean; archived?: boolean };

const on = (v: unknown) => (v === true || v === "true" ? true : undefined);

export const validateTopicSearch = (s: Record<string, unknown>): TopicSearch => ({
  all: on(s.all),
  archived: on(s.archived),
});

const LEVEL_NAMES = ["L0 orientation", "L1 working knowledge", "L2 applied", "L3 expert"];

export function TopicView({ id, search }: { id: number | null; search: TopicSearch }) {
  const all = search.all ?? false;
  const archived = search.archived ?? false;
  const q = useQuery(topicQuery(id, all, archived));
  const navigate = useNavigate();
  const setSearch = (patch: { all?: boolean; archived?: boolean }) =>
    navigate({
      to: ".",
      search: (prev: TopicSearch) => validateTopicSearch({ ...prev, ...patch }),
      replace: true,
    });

  if (q.isPending) return <Skeleton className="h-40" />;
  if (q.isError) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  const page = q.data;
  const topic = page.topic;

  return (
    <div className="space-y-6">
      <div className="space-y-2">
        <Breadcrumbs path={page.breadcrumbs} />
        <div className="flex flex-wrap items-center gap-2">
          <h1 className="mr-2 text-2xl font-semibold">{topic ? topic.name : "Topics"}</h1>
          {topic && <RenameTopicDialog topic={topic} />}
          {topic && <ArchiveTopicDialog topic={topic} />}
          <NewTopicDialog parent={topic} />
        </div>
        {topic && (
          <p className="text-sm text-muted-foreground">
            Target {LEVEL_NAMES[topic.target_level]} · {topic.status === "archived" ? "archived · " : ""}
            {page.subtree_active_questions} active questions here and below
            {topic.description && ` · ${topic.description}`}
          </p>
        )}
      </div>

      <section className="space-y-2">
        <div className="flex items-center gap-3">
          <h2 className="font-medium">{topic ? "Subtopics" : "Your topics"}</h2>
          <div className="ml-auto flex items-center gap-2">
            <Switch
              id="show-archived"
              checked={archived}
              onCheckedChange={(archived) => setSearch({ archived })}
            />
            <Label htmlFor="show-archived" className="text-sm font-normal">
              Show archived
            </Label>
          </div>
        </div>
        {page.children.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            {topic ? "No subtopics." : "No topics yet. Build one with /build-topic in Claude Code."}
          </p>
        ) : (
          <SubtopicCards cards={page.children} />
        )}
      </section>

      {topic && (
        <section className="space-y-2">
          <div className="flex items-center gap-3">
            <h2 className="font-medium">{all ? "All questions in this topic and below" : "Questions"}</h2>
            <div className="ml-auto flex items-center gap-2">
              <Switch id="show-all" checked={all} onCheckedChange={(all) => setSearch({ all })} />
              <Label htmlFor="show-all" className="text-sm font-normal">
                Show all below
              </Label>
            </div>
          </div>
          {page.questions.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              {all ? "No questions yet." : "No questions directly in this topic; they live in its subtopics."}
            </p>
          ) : (
            <QuestionTable rows={page.questions} />
          )}
        </section>
      )}

      {topic && (page.links.length > 0 || page.references.length > 0) && (
        <section className="space-y-1 text-sm">
          {page.links.length > 0 && (
            <p>
              <span className="text-muted-foreground">Links: </span>
              {page.links.map((l, i) => (
                <span key={`${l.topic.id}-${l.kind}`}>
                  {i > 0 && ", "}
                  <Link to="/topics/$topicId" params={{ topicId: String(l.topic.id) }} className="underline">
                    {l.topic.name}
                  </Link>{" "}
                  <span className="text-muted-foreground">({l.kind})</span>
                </span>
              ))}
            </p>
          )}
          {page.references.length > 0 && (
            <p>
              <span className="text-muted-foreground">References: </span>
              {page.references.map((r) => r.title).join(" · ")}
            </p>
          )}
        </section>
      )}
    </div>
  );
}
