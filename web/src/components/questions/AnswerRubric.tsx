import type { Question } from "@/api/generated/Question";

export function AnswerRubric({ question, citation }: { question: Question; citation: string | null }) {
  return (
    <div className="space-y-4 text-sm">
      <div>
        <div className="text-xs uppercase tracking-wide text-muted-foreground">Answer</div>
        <p className="mt-1 whitespace-pre-wrap">{question.answer}</p>
      </div>
      <div>
        <div className="text-xs uppercase tracking-wide text-muted-foreground">
          Rubric · pass at {Math.round(question.pass_threshold * 100)}%
        </div>
        <ul className="mt-1 space-y-1">
          {question.rubric.map((p) => (
            <li key={p.id}>
              <span className="text-muted-foreground">{p.id}</span>{" "}
              {p.essential && <span title="Essential">★ </span>}
              {p.point}
              {p.weight !== 1 && <span className="text-muted-foreground"> (weight {p.weight})</span>}
            </li>
          ))}
        </ul>
      </div>
      {citation && (
        <div>
          <div className="text-xs uppercase tracking-wide text-muted-foreground">Source</div>
          <p className="mt-1">{citation}</p>
        </div>
      )}
    </div>
  );
}
