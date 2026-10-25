import { motion } from "motion/react";
import { type KeyboardEvent, useId } from "react";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { type ColorMode, colorFor } from "@/lib/map/colors";
import { packLayout } from "@/lib/map/layout";
import type { TreeNode } from "@/lib/map/tree";
import { MapTooltip } from "./MapTooltip";
import { summary } from "./summary";

const FONT_PX = 12;
/** Average glyph width at FONT_PX; labels wider than the circle are hidden. */
const CHAR_PX = 7;
/** Circles at least this big also show their question count under the name. */
const COUNT_MIN_R = 32;
const SPRING = { type: "spring", stiffness: 170, damping: 26 } as const;

/** Whether `text` fits across a circle of radius `r`, leaving a margin. */
export function labelFits(text: string, r: number): boolean {
  return r >= FONT_PX * 1.5 && text.length * CHAR_PX <= 2 * r * 0.85;
}

type Props = {
  root: TreeNode;
  width: number;
  height: number;
  color: ColorMode;
  stale: { after: number; full: number };
  /** The day shown, as an ISO date. */
  day: string;
  onZoom(id: string): void;
  onOpenQuestion(id: number): void;
};

/** The root's children as packed circles, with their own children drawn as faint inner rings. */
export function MapView({ root, width, height, color, stale, day, onZoom, onOpenQuestion }: Props) {
  const hatchId = `hatch-${useId().replace(/:/g, "")}`;
  const shapes = packLayout(root, width, height);
  const byId = new Map(root.children.map((n) => [n.id, n]));
  const open = (n: TreeNode) => (n.kind === "topic" ? onZoom(n.id) : onOpenQuestion(n.refId));
  const onKey = (n: TreeNode) => (e: KeyboardEvent) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      open(n);
    }
  };
  const top = shapes.flatMap((s) => {
    const n = s.depth === 1 ? byId.get(s.id) : undefined;
    return n ? [{ s, n }] : [];
  });

  return (
    <svg width={width} height={height} viewBox={`0 0 ${width} ${height}`} className="block select-none">
      <title>{`Map of ${root.label}`}</title>
      <defs>
        <pattern
          id={hatchId}
          width="8"
          height="8"
          patternUnits="userSpaceOnUse"
          patternTransform="rotate(45)"
        >
          <rect width="8" height="8" fill="oklch(0.9 0 0)" />
          <rect width="4" height="8" fill="oklch(0.84 0 0)" />
        </pattern>
      </defs>
      {top.map(({ s, n }) => {
        const c = colorFor(n.measures, color, stale);
        return (
          <Tooltip key={s.id}>
            <TooltipTrigger asChild>
              <motion.circle
                role="button"
                tabIndex={0}
                aria-label={summary(n)}
                initial={false}
                animate={{ cx: s.x, cy: s.y, r: s.r }}
                transition={SPRING}
                cx={s.x}
                cy={s.y}
                r={s.r}
                fill={c.hatched ? `url(#${hatchId})` : c.fill}
                stroke="var(--background)"
                strokeWidth={1.5}
                className="cursor-pointer outline-none transition-colors duration-300 focus-visible:stroke-ring focus-visible:[stroke-width:3]"
                onClick={() => open(n)}
                onKeyDown={onKey(n)}
              />
            </TooltipTrigger>
            <TooltipContent>
              <MapTooltip node={n} day={day} />
            </TooltipContent>
          </Tooltip>
        );
      })}
      {shapes
        .filter((s) => s.depth === 2)
        .map((s) => (
          <motion.circle
            key={s.id}
            aria-hidden
            initial={false}
            animate={{ cx: s.x, cy: s.y, r: s.r }}
            transition={SPRING}
            cx={s.x}
            cy={s.y}
            r={s.r}
            fill="none"
            stroke="white"
            strokeOpacity={0.45}
            className="pointer-events-none"
          />
        ))}
      {top
        .filter(({ s, n }) => labelFits(n.label, s.r))
        .map(({ s, n }) => {
          const ink = colorFor(n.measures, color, stale).text === "light" ? "white" : "oklch(0.25 0 0)";
          const withCount = s.r >= COUNT_MIN_R;
          return (
            <motion.g
              key={s.id}
              aria-hidden
              initial={false}
              animate={{ x: s.x, y: s.y }}
              transition={SPRING}
              className="pointer-events-none"
              fill={ink}
              textAnchor="middle"
              dominantBaseline="central"
            >
              <text y={withCount ? -FONT_PX * 0.6 : 0} fontSize={FONT_PX} fontWeight={600}>
                {n.label}
              </text>
              {withCount && (
                <text y={FONT_PX * 0.75} fontSize={FONT_PX - 1} opacity={0.85}>
                  {n.value}
                </text>
              )}
            </motion.g>
          );
        })}
    </svg>
  );
}
