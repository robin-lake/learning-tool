# Question-writing guide v1

This guide tells Claude how to write questions for Robin's learning tool. `/build-topic` reads it before
generating questions; `/refine-questions` revises it from usefulness ratings. Every question records which
guide version produced it.

## Purpose

Questions exist to build durable, usable knowledge for the long term. The main program has a high bar:
include only what Robin will want to know in ten years. Fewer, better questions beat coverage —
the daily cap on new questions is small (about 3 a day), so every question must earn its place.

## Levels

Levels describe what the learner can *do*, not how obscure the content is.

### L0 — Orientation
- Asks: what is it, why does it matter, what is it for — in plain English.
- Example shape: "In one or two sentences, what does a folded horn do for a subwoofer?"
- Rubric: 1–2 points. `pass_threshold` 0.6.

### L1 — Working knowledge
- Asks: key concepts, how they relate, qualitative trade-offs; enough to follow an expert discussion.
- Example shape: "What happens to low-frequency extension if the horn mouth is made smaller, and why?"
- Rubric: 2–3 points. `pass_threshold` 0.6.

### L2 — Applied
- Asks: use it — apply a formula, work a numeric example, run or read a tool's output, interpret a graph.
- Example shape: "Given a throat area of X and flare constant Y, estimate the cutoff frequency. Show the step."
- Rubric: 3–4 points, including one for a correct worked step, not just the final number. `pass_threshold` 0.7.

### L3 — Expert
- Asks: decide under constraints, diagnose a surprising result, critique a design, defend a trade-off.
- Example shape: "A design measures 6 dB low below 40 Hz. List the likely causes in order and how you'd tell them apart."
- Rubric: 4–6 points, constraint-driven. `pass_threshold` 0.7.

## Item-writing rules

1. **Understand before memorizing.** Only write questions about material that has been explained; never test a bare fact the learner has no context for.
2. **Build on basics.** Write broad, foundational questions first and give them lower priority numbers (introduced sooner). Specific and deep questions come later.
3. **One target per question.** Each question tests one idea. Split compound ideas into several questions. Rubrics add depth to *one* idea; they are not a way to bundle unrelated facts.
4. **No enumerations.** Avoid "list the seven types of X". Ask about each type, or about the distinction that matters.
5. **Precise and unambiguous.** There should be one clear correct answer, so grading is consistent review to review.
6. **Effortful recall, no giveaways.** Don't put the answer's key words in the prompt.
7. **Redundancy is fine.** Approach an important idea from several angles (definition, application, contrast with a neighbor).
8. **Connect ideas.** Elaboration questions ("how does X relate to Y?", "predict what happens if…") are valuable; link the questions and topics they connect.
9. **Cite the source.** Every question generated from a reference records the source and the location (page or section).

## Rubric rules

- Each rubric point is independently checkable from the answer.
- Mark as `essential` the points without which the answer is simply wrong.
- Use weights when one point matters more than the others.
- Write points as what a correct answer contains or demonstrates, not as model wording; accept equivalent phrasing.
- `pass_threshold`: 0.6 for L0–L1, 0.7 for L2–L3, unless there's a reason to differ.

## What's worth asking

- Main program: knowledge that compounds — principles, mental models, trade-offs, and the skills to apply them.
- Skip trivia (exact dates, incidental numbers, names) unless the number itself is something Robin will use.
- Prefer questions that would change a decision Robin might actually make.

## Rephrasing

During review Claude may vary a question's surface wording to prevent rote recall of the prompt.
Keep the target identical: same idea, same rubric, same level.
