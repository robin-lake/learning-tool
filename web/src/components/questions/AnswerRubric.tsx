import { Eye, EyeOff } from "lucide-react";
import { useState } from "react";
import type { Question } from "@/api/generated/Question";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";

/** The answer and rubric, hidden until asked for so the page can be used to self-test. */
export function AnswerRubric({ question, citation }: { question: Question; citation: string | null }) {
  // Keyed by question so moving to another question (e.g. a newer wording) hides it again.
  const [shownFor, setShownFor] = useState<number | null>(null);
  const shown = shownFor === question.id;
  return (
    <div className="space-y-8">
      <section aria-labelledby="answer-heading" className="space-y-3">
        <div className="flex items-center justify-between gap-4">
          <h2 id="answer-heading" className="text-lg font-semibold tracking-tight">
            Answer
          </h2>
          {shown && (
            <Button variant="ghost" size="sm" onClick={() => setShownFor(null)}>
              <EyeOff />
              Hide answer
            </Button>
          )}
        </div>
        {shown ? (
          <>
            <p className="whitespace-pre-wrap text-base leading-relaxed">{question.answer}</p>
            <div className="space-y-2 pt-2">
              <div className="flex items-baseline gap-3">
                <h3 className="font-semibold">Rubric</h3>
                <span className="text-sm text-muted-foreground">
                  Pass at {Math.round(question.pass_threshold * 100)}%
                </span>
              </div>
              <ul className="divide-y rounded-lg border">
                {question.rubric.map((p) => (
                  <li key={p.id} className="flex items-start gap-3 px-4 py-3 leading-relaxed">
                    <span className="mt-0.5 font-mono text-xs text-muted-foreground">{p.id}</span>
                    <span className="flex-1">{p.point}</span>
                    {p.essential && <Badge variant="secondary">Essential</Badge>}
                    {p.weight !== 1 && <span className="text-sm text-muted-foreground">×{p.weight}</span>}
                  </li>
                ))}
              </ul>
            </div>
          </>
        ) : (
          <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed px-6 py-8 text-center">
            <p className="text-sm text-muted-foreground">Try answering it yourself first.</p>
            <Button variant="outline" onClick={() => setShownFor(question.id)}>
              <Eye />
              Show answer
            </Button>
          </div>
        )}
      </section>
      {citation && (
        <section aria-labelledby="source-heading" className="space-y-2">
          <h2 id="source-heading" className="text-lg font-semibold tracking-tight">
            Source
          </h2>
          <p className="border-l-2 border-primary/40 pl-4 leading-relaxed text-muted-foreground">
            {citation}
          </p>
        </section>
      )}
    </div>
  );
}
