import { Link } from "@tanstack/react-router";
import type { TopicCard } from "@/api/generated/TopicCard";
import { Badge } from "@/components/ui/badge";

const pct = (n: number) => Math.round(n * 100);

function levels(counts: [number, number, number, number]) {
  return counts
    .map((n, level) => (n > 0 ? `L${level}: ${n}` : null))
    .filter(Boolean)
    .join(" · ");
}

export function SubtopicCards({ cards }: { cards: TopicCard[] }) {
  return (
    <div className="grid gap-3 sm:grid-cols-2">
      {cards.map((c) => {
        const progress = c.active_questions === 0 ? 0 : pct(c.studied / c.active_questions);
        return (
          <Link
            key={c.id}
            to="/topics/$topicId"
            params={{ topicId: String(c.id) }}
            className="block rounded-lg border p-3 transition-colors hover:bg-accent"
          >
            <div className="flex items-center gap-2">
              <span className="truncate font-medium" title={c.name}>
                {c.name}
              </span>
              {c.status === "archived" && <Badge variant="secondary">archived</Badge>}
              <span className="ml-auto text-muted-foreground">›</span>
            </div>
            <div className="mt-1 text-xs text-muted-foreground">
              {c.active_questions} questions{c.active_questions > 0 && ` (${levels(c.questions_by_level)})`} ·{" "}
              {c.studied} of {c.active_questions} studied
              {c.avg_retrievability !== null && ` · recall ${pct(c.avg_retrievability)}%`}
            </div>
            <div
              role="progressbar"
              aria-label={`${c.name}: ${progress}% studied`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={progress}
              className="mt-2 h-1.5 rounded-full bg-muted"
            >
              <div className="h-1.5 rounded-full bg-heat-3" style={{ width: `${progress}%` }} />
            </div>
          </Link>
        );
      })}
    </div>
  );
}
