import { format, parseISO } from "date-fns";
import { Star } from "lucide-react";
import { useState } from "react";
import type { RatingRow } from "@/api/generated/RatingRow";
import { useRateQuestion } from "@/api/queries";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { cn } from "@/lib/utils";

export function RatingBox({ questionId, ratings }: { questionId: number; ratings: RatingRow[] }) {
  const [stars, setStars] = useState(0);
  const [feedback, setFeedback] = useState("");
  const rate = useRateQuestion();
  return (
    <div className="space-y-2">
      <div className="text-xs uppercase tracking-wide text-muted-foreground">
        How useful is this question?
      </div>
      <div role="radiogroup" aria-label="Usefulness" className="flex gap-1">
        {[1, 2, 3, 4, 5].map((n) => (
          <button
            key={n}
            type="button"
            role="radio"
            aria-checked={stars === n}
            aria-label={`${n} of 5`}
            onClick={() => setStars(n)}
            className="rounded p-0.5 focus-visible:ring-2 focus-visible:ring-ring"
          >
            <Star
              className={cn("size-5", n <= stars ? "fill-amber-400 text-amber-500" : "text-muted-foreground")}
            />
          </button>
        ))}
      </div>
      <Label htmlFor={`rating-feedback-${questionId}`} className="text-sm font-normal">
        Why? (optional)
      </Label>
      <Textarea
        id={`rating-feedback-${questionId}`}
        value={feedback}
        onChange={(e) => setFeedback(e.target.value)}
        placeholder="e.g. too trivial, unclear wording, very useful"
        className="min-h-16"
      />
      <Button
        size="sm"
        disabled={stars === 0 || rate.isPending}
        onClick={() =>
          rate.mutate(
            { id: questionId, usefulness: stars, feedback: feedback.trim() || undefined },
            {
              onSuccess: () => {
                setStars(0);
                setFeedback("");
              },
            },
          )
        }
      >
        Save rating
      </Button>
      {rate.isSuccess && (
        <p className="text-sm text-muted-foreground">Saved. /refine-questions will use it.</p>
      )}
      {rate.error && <p className="text-sm text-destructive">{rate.error.message}</p>}
      {ratings.length > 0 && (
        <ul className="pt-1 text-sm text-muted-foreground">
          {ratings.map((r) => (
            <li key={r.id}>
              {format(parseISO(r.rated_at), "d MMM")}: {r.usefulness}/5{r.feedback && `, “${r.feedback}”`}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
