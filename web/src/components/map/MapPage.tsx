import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { format, parseISO } from "date-fns";
import { Fragment, useLayoutEffect, useState } from "react";
import type { TimelineNode } from "@/api/generated/TimelineNode";
import { timelineQuery } from "@/api/queries";
import { ErrorState } from "@/components/ErrorState";
import { Skeleton } from "@/components/ui/skeleton";
import type { ColorMode } from "@/lib/map/colors";
import { buildTree, type SizeMode } from "@/lib/map/tree";
import { COLOR_MODES, MapControls } from "./MapControls";
import { MapLegend } from "./MapLegend";
import { MapView } from "./MapView";
import { PlaybackBar } from "./PlaybackBar";

export type MapSearch = { topic?: string; day?: string; color?: ColorMode; mode?: SizeMode };

const isColor = (v: unknown): v is ColorMode => COLOR_MODES.some((m) => m.value === v);

/** Keeps only valid values, and omits defaults so the URL stays short. */
export const validateMapSearch = (s: Record<string, unknown>): MapSearch => ({
  ...(typeof s.topic === "string" && /^t\d+$/.test(s.topic) ? { topic: s.topic } : {}),
  ...(typeof s.day === "string" && /^\d{4}-\d{2}-\d{2}$/.test(s.day) ? { day: s.day } : {}),
  ...(isColor(s.color) && s.color !== "combined" ? { color: s.color } : {}),
  ...(s.mode === "fixed" ? { mode: "fixed" as const } : {}),
});

const FALLBACK = { width: 800, height: 560 };

/** The container's size, falling back to a default before layout (and in tests). A callback ref,
 * because the container only mounts once the timeline has loaded. */
function useSize() {
  const [el, setEl] = useState<HTMLDivElement | null>(null);
  const [size, setSize] = useState(FALLBACK);
  useLayoutEffect(() => {
    if (!el) return;
    const measure = () => {
      const width = el.clientWidth;
      if (width > 0) setSize({ width, height: Math.max(360, Math.min(720, Math.round(width * 0.7))) });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [el]);
  return [setEl, size] as const;
}

function path(nodes: TimelineNode[], id: string | undefined): TimelineNode[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const out: TimelineNode[] = [];
  let at = id ? byId.get(id) : undefined;
  while (at) {
    out.unshift(at);
    at = at.parent ? byId.get(at.parent) : undefined;
  }
  return out;
}

export function MapPage({ search }: { search: MapSearch }) {
  const q = useQuery(timelineQuery());
  const navigate = useNavigate();
  const [box, size] = useSize();
  const setSearch = (patch: Partial<Record<keyof MapSearch, string | undefined>>, replace = true) =>
    navigate({ to: "/map", search: (prev: MapSearch) => validateMapSearch({ ...prev, ...patch }), replace });

  if (q.isPending) return <Skeleton className="h-96" />;
  if (q.isError) return <ErrorState error={q.error} onRetry={() => q.refetch()} />;
  const tl = q.data;
  const color = search.color ?? "combined";
  const mode = search.mode ?? "growing";
  const found = search.day ? tl.days.indexOf(search.day) : -1;
  const index = found >= 0 && found < tl.days.length - 1 ? found : null;
  const day = index === null ? tl.today : tl.days[index];
  const stale = { after: tl.stale_after_days, full: tl.stale_full_days };

  const zoomed = search.topic ? buildTree(tl.nodes, search.topic, index, mode) : null;
  const unknownTopic = search.topic !== undefined && zoomed === null;
  const root = zoomed ?? buildTree(tl.nodes, null, index, mode);
  const crumbs = zoomed ? path(tl.nodes, zoomed.id) : [];

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div className="space-y-1">
          <nav aria-label="Breadcrumb" className="text-sm">
            {crumbs.length === 0 ? (
              <span aria-current="page">Topics</span>
            ) : (
              <button
                type="button"
                className="underline-offset-2 hover:underline"
                onClick={() => setSearch({ topic: undefined }, false)}
              >
                Topics
              </button>
            )}
            {crumbs.map((t, i) => (
              <Fragment key={t.id}>
                <span className="text-muted-foreground"> › </span>
                {i === crumbs.length - 1 ? (
                  <span aria-current="page">{t.label}</span>
                ) : (
                  <button
                    type="button"
                    className="underline-offset-2 hover:underline"
                    onClick={() => setSearch({ topic: t.id }, false)}
                  >
                    {t.label}
                  </button>
                )}
              </Fragment>
            ))}
          </nav>
          <h1 className="text-2xl font-semibold">{zoomed ? zoomed.label : "Knowledge map"}</h1>
        </div>
        <MapControls
          color={color}
          mode={mode}
          onColor={(c) => setSearch({ color: c })}
          onMode={(m) => setSearch({ mode: m })}
        />
      </div>
      {unknownTopic && (
        <p role="status" className="text-sm text-muted-foreground">
          That topic isn't on the map (it may be archived or not exist). Showing all topics.
        </p>
      )}
      <MapLegend mode={color} stale={stale} />
      <PlaybackBar
        days={tl.days}
        index={index}
        onIndex={(i) => setSearch({ day: i === null ? undefined : tl.days[i] })}
      />
      <div ref={box} className="w-full">
        {root && root.children.length > 0 ? (
          <MapView
            root={root}
            width={size.width}
            height={size.height}
            color={color}
            stale={stale}
            day={day}
            onZoom={(id) => setSearch({ topic: id }, false)}
            onOpenQuestion={(id) =>
              navigate({ to: "/questions/$questionId", params: { questionId: String(id) } })
            }
          />
        ) : (
          <p className="py-16 text-center text-muted-foreground">
            {tl.nodes.length === 0 && !zoomed
              ? "No topics yet. Build one with /build-topic in Claude Code."
              : `Nothing here yet on ${format(parseISO(day), "d MMM yyyy")}`}
          </p>
        )}
      </div>
    </div>
  );
}
