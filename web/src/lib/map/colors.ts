import { freshness } from "./staleness";

export type ColorMode = "combined" | "recall" | "durability" | "score" | "staleness";

export type Measures = {
  studied: number;
  recall: number | null;
  durability: number | null;
  lastScore: number | null;
  daysSince: number | null;
};

export type Fill = { fill: string; text: "light" | "dark"; hatched: boolean };

/** The ranges the combined view maps its measures into (OKLCH). */
export const BANDS = {
  hueWeak: 25,
  hueStrong: 255,
  lightFragile: 0.85,
  lightDurable: 0.45,
  durFragileDays: 1,
  durDurableDays: 90,
  chromaFresh: 0.15,
  chromaFloor: 0.06,
};

const SINGLE = { l: 0.65, c: 0.13 };
const NEVER = "oklch(0.9 0 0)";
const LIGHT_TEXT_BELOW = 0.62;

const clamp01 = (x: number) => Math.max(0, Math.min(1, x));

/** Red (0) → purple → blue (1): decreasing hue through 0/360, avoiding green. */
export function hueFor(t: number): number {
  return (BANDS.hueWeak - (BANDS.hueWeak + 360 - BANDS.hueStrong) * clamp01(t) + 360) % 360;
}

/** Stability in days on a log scale: 0 at `durFragileDays`, 1 at `durDurableDays`. */
function durabilityScale(days: number | null): number {
  if (days === null || days <= 0) return 0;
  return clamp01(
    Math.log(days / BANDS.durFragileDays) / Math.log(BANDS.durDurableDays / BANDS.durFragileDays),
  );
}

const fmt = (l: number, c: number, h: number) => `oklch(${+l.toFixed(4)} ${+c.toFixed(4)} ${+h.toFixed(4)})`;

export function colorFor(m: Measures, mode: ColorMode, stale: { after: number; full: number }): Fill {
  if (m.studied === 0) return { fill: NEVER, text: "dark", hatched: true };
  const fresh = freshness(m.daysSince, stale.after, stale.full);
  let l = SINGLE.l;
  let c = SINGLE.c;
  let t: number;
  switch (mode) {
    case "combined":
      t = m.lastScore ?? 0;
      l = BANDS.lightFragile - (BANDS.lightFragile - BANDS.lightDurable) * durabilityScale(m.durability);
      c = BANDS.chromaFloor + (BANDS.chromaFresh - BANDS.chromaFloor) * fresh;
      break;
    case "recall":
      t = m.recall ?? 0;
      break;
    case "durability":
      t = durabilityScale(m.durability);
      break;
    case "score":
      t = m.lastScore ?? 0;
      break;
    case "staleness":
      t = fresh;
      break;
  }
  return { fill: fmt(l, c, hueFor(t)), text: l < LIGHT_TEXT_BELOW ? "light" : "dark", hatched: false };
}

/** Reads back an `oklch(L C H)` string produced by `colorFor`. */
export function parseOklch(s: string): { l: number; c: number; h: number } {
  const [l, c, h] = s
    .replace(/^oklch\(|\)$/g, "")
    .split(/\s+/)
    .map(Number);
  return { l, c, h };
}
