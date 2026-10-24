---
name: refine-questions
description: Improve Robin's learning-tool question-writing guide from usefulness ratings and feedback — classify the feedback, propose a new guide version with good/bad examples, and propose rewrites or retirements for poorly rated questions. Use when Robin says /refine-questions or asks to improve how questions are written.
---

# Refine the question-writing guide

Robin rates questions 1–5 during study, with feedback in their own words. This skill turns that
feedback into a better guide for `/build-topic`, and fixes the rated questions themselves — the same
idea as distilling review-comment history into a review checklist.

## Flow

1. Call `mcp__learning-tool__list_ratings` (unprocessed only) and `mcp__learning-tool__get_guide`.
   If there are no unprocessed ratings, say so and stop.
2. **Classify** each rating that has no classification, from Robin's words, and save it with
   `mcp__learning-tool__classify_rating`:
   - `question_quality` — badly made: wording, ambiguity, triviality, wrong level, rubric too strict/loose.
   - `knowledge_value` — the knowledge isn't worth long-term study.
   - `positive` — a good question; useful as an example.
   If a piece of feedback is genuinely ambiguous, ask Robin rather than guess.
3. **Propose a new guide version.** Show Robin a short diff-style summary:
   - `question_quality` patterns → changes to *how* questions are written (level criteria, rubric rules,
     phrasing rules).
   - `knowledge_value` patterns → changes to *what* gets chosen ("what's worth asking").
   - High ratings → good examples; low ratings → bad examples, each with a one-line reason.
   For each change, name the ratings that drove it. Keep the guide concise — refine, don't bloat.
4. **Propose actions on the rated questions** (one line each):
   - rewrite as a revision (`update_question`) — for `question_quality`,
   - retire (`set_question_status` → `retired`) — for `knowledge_value`, or when it can't be saved,
   - leave as is.
5. **Only after Robin approves:** call `mcp__learning-tool__save_guide_version` with the full new
   `content`, a `change_summary`, the new `examples`, and all `processed_rating_ids` you accounted for.
   The current examples carry forward automatically; a new verdict on the same question replaces the
   old one, and `removed_example_question_ids` drops examples that no longer help.
   Then apply the approved question actions. Report what changed in two or three lines.
