import { type ColorMode, colorFor, type Measures, NEVER } from "@/lib/map/colors";

type Stale = { after: number; full: number };

const base: Measures = { studied: 1, recall: 0.5, durability: 9, lastScore: 0.5, daysSince: 0 };
const STEPS = [0, 0.25, 0.5, 0.75, 1];

/** What the hue scale means in each mode, from its weak (red) to its strong (blue) end. */
function scale(
  mode: ColorMode,
  stale: Stale,
): { title: string; weak: string; strong: string; at: (t: number) => Measures } {
  switch (mode) {
    case "combined":
      return { title: "Last score", weak: "0%", strong: "100%", at: (t) => ({ ...base, lastScore: t }) };
    case "score":
      return { title: "Last score", weak: "0%", strong: "100%", at: (t) => ({ ...base, lastScore: t }) };
    case "recall":
      return { title: "Recall now", weak: "0%", strong: "100%", at: (t) => ({ ...base, recall: t }) };
    case "durability":
      return {
        title: "Durability",
        weak: "1 day",
        strong: "90+ days",
        at: (t) => ({ ...base, durability: 90 ** t }),
      };
    case "staleness":
      return {
        title: "Last studied",
        weak: `${stale.full}+ days ago`,
        strong: `within ${stale.after} days`,
        at: (t) => ({ ...base, daysSince: stale.full - t * (stale.full - stale.after) }),
      };
  }
}

function Swatch({ fill }: { fill: string }) {
  return (
    <span
      aria-hidden
      className="inline-block size-3 rounded-full border border-black/10 align-middle"
      style={{ background: fill }}
    />
  );
}

/** Explains the current coloring. */
export function MapLegend({ mode, stale }: { mode: ColorMode; stale: Stale }) {
  const s = scale(mode, stale);
  const fill = (m: Measures) => colorFor(m, mode, stale).fill;
  const gradient = `linear-gradient(90deg, ${STEPS.map((t) => fill(s.at(t))).join(", ")})`;
  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
      <span className="flex items-center gap-1.5">
        <span className="font-medium text-foreground">{s.title}</span>
        <span>{s.weak}</span>
        <span aria-hidden className="inline-block h-2.5 w-28 rounded-full" style={{ background: gradient }} />
        <span>{s.strong}</span>
      </span>
      {mode === "combined" && (
        <>
          <span className="flex items-center gap-1.5">
            <Swatch fill={fill({ ...base, lastScore: 1, durability: 1 })} />
            <Swatch fill={fill({ ...base, lastScore: 1, durability: 90 })} />
            <span>Pale = fragile, deep = durable</span>
          </span>
          <span className="flex items-center gap-1.5">
            <Swatch fill={fill({ ...base, lastScore: 1, daysSince: 0 })} />
            <Swatch fill={fill({ ...base, lastScore: 1, daysSince: stale.full })} />
            <span>{`Faded = not studied in ${stale.after}+ days`}</span>
          </span>
        </>
      )}
      <span className="flex items-center gap-1.5">
        <Swatch fill={NEVER} />
        <span>Not studied</span>
      </span>
    </div>
  );
}
