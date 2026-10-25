import { Button } from "@/components/ui/button";
import type { ColorMode } from "@/lib/map/colors";
import type { SizeMode } from "@/lib/map/tree";

export const COLOR_MODES: { value: ColorMode; label: string }[] = [
  { value: "combined", label: "Combined" },
  { value: "recall", label: "Recall now" },
  { value: "durability", label: "Durability" },
  { value: "score", label: "Last score" },
  { value: "staleness", label: "Staleness" },
];

const SIZE_MODES: { value: SizeMode; label: string; title: string }[] = [
  { value: "growing", label: "Growing", title: "Circles grow as questions are added" },
  { value: "fixed", label: "Fixed", title: "Today's sizes on every day; only the colors change" },
];

type Props = {
  color: ColorMode;
  mode: SizeMode;
  onColor(c: ColorMode): void;
  onMode(m: SizeMode): void;
};

export function MapControls({ color, mode, onColor, onMode }: Props) {
  return (
    <div className="flex flex-wrap items-center gap-4 text-sm">
      <label className="flex items-center gap-2">
        <span className="text-muted-foreground">Color by</span>
        <select
          value={color}
          onChange={(e) => onColor(e.target.value as ColorMode)}
          className="h-8 rounded-md border bg-background px-2"
        >
          {COLOR_MODES.map((m) => (
            <option key={m.value} value={m.value}>
              {m.label}
            </option>
          ))}
        </select>
      </label>
      <fieldset className="flex items-center gap-1">
        <legend className="sr-only">Circle sizes</legend>
        {SIZE_MODES.map((m) => (
          <Button
            key={m.value}
            size="sm"
            variant={mode === m.value ? "secondary" : "ghost"}
            aria-pressed={mode === m.value}
            title={m.title}
            onClick={() => onMode(m.value)}
          >
            {m.label}
          </Button>
        ))}
      </fieldset>
    </div>
  );
}
