import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import type { Question } from "@/api/generated/Question";
import type { QuestionPatch } from "@/api/generated/QuestionPatch";
import { useUpdateQuestion } from "@/api/queries";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { validateRubric } from "@/lib/rubric";
import { RubricEditor } from "./RubricEditor";

/** Only the fields that differ, so saving unchanged fields doesn't create a revision. */
function changes(
  q: Question,
  draft: Required<Pick<QuestionPatch, "prompt" | "answer" | "level" | "pass_threshold" | "rubric">>,
) {
  const patch: QuestionPatch = {};
  if (draft.prompt !== q.prompt) patch.prompt = draft.prompt;
  if (draft.answer !== q.answer) patch.answer = draft.answer;
  if (draft.level !== q.level) patch.level = draft.level;
  if (draft.pass_threshold !== q.pass_threshold) patch.pass_threshold = draft.pass_threshold;
  if (JSON.stringify(draft.rubric) !== JSON.stringify(q.rubric)) patch.rubric = draft.rubric;
  return patch;
}

export function EditQuestionDialog({ question }: { question: Question }) {
  const [open, setOpen] = useState(false);
  const [prompt, setPrompt] = useState(question.prompt);
  const [answer, setAnswer] = useState(question.answer);
  const [level, setLevel] = useState(question.level);
  const [threshold, setThreshold] = useState(Math.round(question.pass_threshold * 100));
  const [rubric, setRubric] = useState(question.rubric);
  const update = useUpdateQuestion();
  const navigate = useNavigate();

  const reset = () => {
    setPrompt(question.prompt);
    setAnswer(question.answer);
    setLevel(question.level);
    setThreshold(Math.round(question.pass_threshold * 100));
    setRubric(question.rubric);
    update.reset();
  };
  const errors = [
    ...(prompt.trim() === "" ? ["The question can't be empty."] : []),
    ...(answer.trim() === "" ? ["The answer can't be empty."] : []),
    ...validateRubric(rubric, threshold / 100),
  ];
  const patch = changes(question, { prompt, answer, level, pass_threshold: threshold / 100, rubric });
  const unchanged = Object.keys(patch).length === 0;

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (o) reset();
      }}
    >
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          Edit
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Edit question</DialogTitle>
          <DialogDescription>
            Rewording creates a new version; history is kept and the schedule carries over.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1">
            <Label htmlFor="q-prompt">Question</Label>
            <Textarea id="q-prompt" value={prompt} onChange={(e) => setPrompt(e.target.value)} />
          </div>
          <div className="space-y-1">
            <Label htmlFor="q-answer">Answer</Label>
            <Textarea id="q-answer" value={answer} onChange={(e) => setAnswer(e.target.value)} />
          </div>
          <div className="flex gap-4">
            <div className="space-y-1">
              <Label htmlFor="q-level">Level</Label>
              <Select value={String(level)} onValueChange={(v) => setLevel(Number(v))}>
                <SelectTrigger id="q-level" className="w-28">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {[0, 1, 2, 3].map((l) => (
                    <SelectItem key={l} value={String(l)}>
                      L{l}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-1">
              <Label htmlFor="q-threshold">Pass at (%)</Label>
              <Input
                id="q-threshold"
                type="number"
                min={1}
                max={100}
                value={threshold}
                onChange={(e) => setThreshold(Number(e.target.value))}
                className="w-24"
              />
            </div>
          </div>
          <div className="space-y-1">
            <div className="text-sm font-medium">Rubric</div>
            <RubricEditor points={rubric} onChange={setRubric} />
          </div>
          {errors.length > 0 && (
            <ul className="text-sm text-destructive">
              {errors.map((e) => (
                <li key={e}>{e}</li>
              ))}
            </ul>
          )}
          {update.error && <p className="text-sm text-destructive">{update.error.message}</p>}
        </div>
        <DialogFooter>
          <Button
            disabled={errors.length > 0 || unchanged || update.isPending}
            onClick={() =>
              update.mutate(
                { id: question.id, patch },
                {
                  onSuccess: (q) => {
                    setOpen(false);
                    navigate({ to: "/questions/$questionId", params: { questionId: String(q.id) } });
                  },
                },
              )
            }
          >
            Save new version
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
