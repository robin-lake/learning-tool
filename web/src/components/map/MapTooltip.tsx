import { format, parseISO, subDays } from "date-fns";
import type { TreeNode } from "@/lib/map/tree";
import { pct, plural } from "./summary";

/** Every measure for one circle as of `day` (an ISO date). */
export function MapTooltip({ node, day }: { node: TreeNode; day: string }) {
  const m = node.measures;
  return (
    <div className="space-y-0.5">
      <div className="font-medium">{node.label}</div>
      <div>{`${plural(node.value, "question")}, ${m.studied} studied`}</div>
      {m.studied === 0 ? (
        <div>Not studied yet</div>
      ) : (
        <>
          {m.recall !== null && <div>{`Recall now: ${pct(m.recall)}`}</div>}
          {m.durability !== null && <div>{`Durability: ${plural(Math.round(m.durability), "day")}`}</div>}
          {m.lastScore !== null && <div>{`Last score: ${pct(m.lastScore)}`}</div>}
          {m.daysSince !== null && (
            <div>{`Last studied: ${format(subDays(parseISO(day), m.daysSince), "d MMM yyyy")} (${
              m.daysSince === 0 ? "that day" : `${plural(m.daysSince, "day")} ago`
            })`}</div>
          )}
        </>
      )}
      <div>{`Due: ${node.due}`}</div>
      {node.targetLevel !== null && <div>{`Target: L${node.targetLevel}`}</div>}
    </div>
  );
}
