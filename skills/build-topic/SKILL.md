---
name: build-topic
description: Build or extend a learning topic in Robin's learning-tool — clarify the goal and depth, outline subtopics broad to specific, find and store good references (PDFs, articles, docs), and generate prioritized L0–L3 questions with rubrics. Use when Robin says /build-topic, wants to learn a new subject, or asks for questions on a topic.
---

# Build a topic

You help Robin turn a subject into a well-structured topic with good references and questions,
using the `learning-tool` MCP tools. Questions go into the long-term main program, which has a high
bar ("will I want to know this in 10 years?") and a small daily cap on new questions (about 3/day),
so **fewer, better questions beat coverage**.

## 1. Goal and depth

Unless Robin already said, ask in one or two short questions: why this subject, and how deep
(target level). Levels:

- **L0 Orientation** — what it is, why it matters, in plain English.
- **L1 Working knowledge** — concepts and qualitative trade-offs; can follow an expert discussion.
- **L2 Applied** — formulas, worked calculations, using the tools.
- **L3 Expert** — design decisions under constraints, diagnosis, critique.

Call `mcp__learning-tool__list_topics` to see what exists. Decide where the subject sits: under an
existing topic or as a new top-level topic. Note overlaps worth cross-linking.

## 2. Outline (metalearning)

Propose a short subtopic outline, **broad to specific**, each with a target level. Keep it small; it can
grow later. Let Robin adjust it, then create the topics with `mcp__learning-tool__create_topic`
(in outline order, so `position` follows it).

## 3. References

Find high-quality sources: textbooks and lecture notes, well-regarded articles, official
documentation, best-practice guides. Prefer a few excellent sources over many weak ones. Also accept
files Robin provides. For each reference:

1. Download into a temporary directory of its own (e.g. `mktemp -d`), never into the repo.
2. Extract text:
   - PDF: `pdftotext -layout in.pdf out.txt` (keeps page breaks, so locators become "p. N").
     If that yields almost no text, the PDF is scanned: run `ocrmypdf in.pdf ocr.pdf` first.
   - Web article: save the readable content as Markdown with its headings (locators become heading paths).
3. Write a 2–4 sentence `summary`: the essence and what it's good for.
4. Call `mcp__learning-tool__add_reference` with absolute paths (`source_file`, `text_file`), `kind`,
   `title`, `authors`, `url`, `summary`, and the relevant `topic_ids`.

Respect access: don't try to get around paywalls; ask Robin for files instead. A source Robin doesn't
have yet (e.g. a book) can be stored as metadata only: `title`, `authors`, `kind`, `url`, `summary`.

**Before adding, check for an existing entry** with `mcp__learning-tool__search_references` or the
topic's references (`get_topic`, then `get_reference`). When Robin later provides the file for a
reference that's already stored, extract its text as above and call
`mcp__learning-tool__attach_reference_file` with the reference `id`, `source_file` and `text_file`
instead of adding a duplicate. That keeps its id, so questions citing it stay linked. Pass
`replace: true` only to swap files it already has.

## 4. Extract and write questions

1. Call `mcp__learning-tool__get_guide` and follow it — it holds the current level criteria, rules
   and good/bad examples, refined from Robin's ratings. Note its version id.
2. For each subtopic, use `mcp__learning-tool__search_references` and `read_reference` to pull out the
   essential ideas. Write questions only for what matters most:
   - **Broad before specific**, L0 before deeper levels — set `priority` so the most foundational
     questions come first (lower = sooner).
   - One target per question; no lists or enumerations; precise; no giveaways in the prompt.
   - Each question has an `answer`, a `rubric` (points with optional `weight` and `essential`), a
     `pass_threshold` (0.6 for L0–L1, 0.7 for L2–L3 by default), `source_id` + `source_locator`, and
     `guide_version_id`.
   - Include some elaboration questions that connect ideas across subtopics.
3. Record connections with `mcp__learning-tool__link_topics` / `link_questions` (weight = how closely
   related, 0–1).

## 5. Add

- **Default:** call `mcp__learning-tool__add_questions` with status `active` (all-or-nothing per call).
- **If Robin asked to review first:** show the draft compactly (level, prompt, rubric points), add only
  what Robin approves; anything to keep for later can go in as `proposed`.
- Report the **load forecast** from the result in plain words, e.g. "18 questions ≈ 6 days of new-question
  capacity, about 5 extra reviews a day over the next month."

## 6. Close

Summarize in a few lines: topics created, references added, questions added, and the forecast.
Suggest `/study` when there's capacity today.
