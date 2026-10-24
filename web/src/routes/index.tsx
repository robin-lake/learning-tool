import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { format, parseISO, subDays } from "date-fns";
import { activityQuery, homeQuery } from "@/api/queries";
import { ErrorState } from "@/components/ErrorState";
import { ComingUp } from "@/components/home/ComingUp";
import { Heatmap } from "@/components/home/Heatmap";
import { NeedsAttention } from "@/components/home/NeedsAttention";
import { TodayTiles } from "@/components/home/TodayTiles";
import { Skeleton } from "@/components/ui/skeleton";

export const Route = createFileRoute("/")({ component: Home });

function Home() {
  const home = useQuery(homeQuery());
  const today = home.data?.today.study_day ?? "";
  const from = today ? format(subDays(parseISO(today), 371), "yyyy-MM-dd") : "";
  const activity = useQuery({ ...activityQuery(from, today), enabled: today !== "" });

  if (home.isPending) {
    return (
      <div className="grid gap-4 sm:grid-cols-3">
        <Skeleton className="h-28" />
        <Skeleton className="h-28" />
        <Skeleton className="h-28" />
      </div>
    );
  }
  if (home.isError) return <ErrorState error={home.error} onRetry={() => home.refetch()} />;

  return (
    <div className="space-y-6">
      <TodayTiles summary={home.data} />
      <section>
        <h2 className="mb-2 text-sm font-medium text-muted-foreground">Last 12 months</h2>
        {activity.isError ? (
          <ErrorState error={activity.error} onRetry={() => activity.refetch()} />
        ) : (
          <Heatmap activity={activity.data ?? []} today={today} />
        )}
      </section>
      <div className="grid gap-4 md:grid-cols-2">
        <ComingUp summary={home.data} />
        <NeedsAttention summary={home.data} />
      </div>
    </div>
  );
}
