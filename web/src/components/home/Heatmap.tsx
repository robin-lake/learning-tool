import { format, parseISO } from "date-fns";
import type { DayActivity } from "@/api/generated/DayActivity";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { buildHeatmap, gradeLine, type HeatCell } from "@/lib/heatmap";
import { cn } from "@/lib/utils";

function label(c: HeatCell) {
  const day = format(parseISO(c.date), "d MMM yyyy");
  return `${day}: ${c.count === 0 ? "no questions" : `${c.count} question${c.count === 1 ? "" : "s"}`}`;
}

const heat = ["bg-heat-0", "bg-heat-1", "bg-heat-2", "bg-heat-3", "bg-heat-4"] as const;

/** One column per week (Sunday at the top), one focusable button per day with a tooltip. */
export function Heatmap({ activity, today }: { activity: DayActivity[]; today: string }) {
  const weeks = buildHeatmap(activity, today);
  return (
    <div
      role="group"
      aria-label="Questions answered per day, last 12 months"
      className="flex gap-[3px] overflow-x-auto pb-1"
    >
      {weeks.map((week) => (
        <div key={week[0].date} className="flex flex-col gap-[3px]">
          {week.map((c) => (
            <Tooltip key={c.date}>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  aria-label={label(c)}
                  className={cn(
                    "size-[11px] rounded-[2px] outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    heat[c.level],
                    c.isToday && "ring-2 ring-amber-500",
                  )}
                />
              </TooltipTrigger>
              <TooltipContent>
                <div className="font-medium">{format(parseISO(c.date), "EEE d MMM yyyy")}</div>
                <div>{c.count === 0 ? "No questions" : `${c.count} question${c.count === 1 ? "" : "s"}`}</div>
                {c.count > 0 && <div>{gradeLine(c.byGrade)}</div>}
                {c.topics.length > 0 && <div>Topics: {c.topics.join(", ")}</div>}
              </TooltipContent>
            </Tooltip>
          ))}
        </div>
      ))}
    </div>
  );
}
