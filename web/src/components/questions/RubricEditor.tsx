import { ArrowDown, ArrowUp, X } from "lucide-react";
import type { RubricPoint } from "@/api/generated/RubricPoint";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { nextPointId } from "@/lib/rubric";

export function RubricEditor({
  points,
  onChange,
}: {
  points: RubricPoint[];
  onChange: (p: RubricPoint[]) => void;
}) {
  const set = (i: number, patch: Partial<RubricPoint>) =>
    onChange(points.map((p, j) => (j === i ? { ...p, ...patch } : p)));
  const move = (i: number, by: number) => {
    const next = [...points];
    [next[i], next[i + by]] = [next[i + by], next[i]];
    onChange(next);
  };
  return (
    <div className="space-y-2">
      {points.map((p, i) => (
        <div key={p.id} className="flex items-start gap-2 rounded-md border p-2">
          <span className="mt-2 w-7 text-xs text-muted-foreground">{p.id}</span>
          <div className="flex-1 space-y-2">
            <Input
              aria-label={`Point ${i + 1} text`}
              value={p.point}
              onChange={(e) => set(i, { point: e.target.value })}
            />
            <div className="flex items-center gap-3 text-sm">
              <Label className="font-normal">
                Weight
                <Input
                  aria-label={`Point ${i + 1} weight`}
                  type="number"
                  min={0}
                  step={0.5}
                  value={p.weight}
                  onChange={(e) => set(i, { weight: Number(e.target.value) })}
                  className="h-8 w-20"
                />
              </Label>
              <Label className="font-normal">
                <Switch
                  aria-label={`Point ${i + 1} essential`}
                  checked={p.essential}
                  onCheckedChange={(essential) => set(i, { essential })}
                />
                Essential
              </Label>
            </div>
          </div>
          <div className="flex flex-col">
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Move point ${i + 1} up`}
              disabled={i === 0}
              onClick={() => move(i, -1)}
            >
              <ArrowUp />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Move point ${i + 1} down`}
              disabled={i === points.length - 1}
              onClick={() => move(i, 1)}
            >
              <ArrowDown />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Remove point ${i + 1}`}
              onClick={() => onChange(points.filter((_, j) => j !== i))}
            >
              <X />
            </Button>
          </div>
        </div>
      ))}
      <Button
        variant="outline"
        size="sm"
        onClick={() =>
          onChange([...points, { id: nextPointId(points), point: "", weight: 1, essential: false }])
        }
      >
        + Point
      </Button>
    </div>
  );
}
