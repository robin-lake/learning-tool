import type { HomeSummary } from "@/api/generated/HomeSummary";
import { Card, CardContent } from "@/components/ui/card";
import { gradeLine } from "@/lib/heatmap";

const days = (n: number) => `${n} day${n === 1 ? "" : "s"}`;

function Tile({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <Card className="py-4">
      <CardContent className="px-4">
        <div className="text-xs uppercase tracking-wide text-muted-foreground">{label}</div>
        <div className="mt-1 text-3xl font-semibold">{value}</div>
        <div className="mt-1 text-sm text-muted-foreground">{detail}</div>
      </CardContent>
    </Card>
  );
}

export function TodayTiles({ summary }: { summary: HomeSummary }) {
  const t = summary.today;
  return (
    <div className="grid gap-4 sm:grid-cols-3">
      <Tile
        label="Answered today"
        value={String(t.answered_today)}
        detail={gradeLine(summary.by_grade_today)}
      />
      <Tile
        label="Remaining today"
        value={String(t.remaining_today)}
        detail={`${t.due_tomorrow} due tomorrow · run /study in Claude Code`}
      />
      <Tile label="Streak" value={days(t.streak)} detail={`Best: ${days(summary.best_streak)}`} />
    </div>
  );
}
