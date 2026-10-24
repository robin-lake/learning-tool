# Learning Tool

A personal spaced-repetition learning tool (Anki-like). Claude asks the user questions, the user's answers are graded (again / hard / good / easy), and a scheduler decides when each question comes back. A local web app shows progress and a map of what the user knows.

**Status: planning.** No code yet. Design docs go in `docs/superpowers/specs/`, implementation plans in `docs/superpowers/plans/`. Update this file when decisions are made — don't let it drift from reality.

## Decided

- **Backend:** Rust.
- **Storage:** a local SQLite database. Single user, runs on the user's machine.
- **Frontend:** React + TypeScript (Vite, TanStack, shadcn). The goal is a tool to actually use, so pick the mature ecosystem. API types are generated from the Rust structs (e.g. `ts-rs` / `specta`) rather than hand-written. The 3D stretch goal may be a Rust/WASM module embedded in the React app.
- **Quizzing happens in Claude Code**, not in the web app. Claude reads due questions and records grades against the local database (likely via an MCP server sharing a core crate with the web backend). The web app is for tracking, visualization, and project features.
- **Claude writes questions with the user.** A "build a topic" skill: Claude finds references (PDFs, articles, textbooks, best-practice guides), stores them, extracts their essence, and generates questions. New questions go straight into the main program by default; reviewing them first is opt-in.
- **References:** metadata in SQLite; original files in a folder beside the DB; text extracted once to a text/Markdown file (with page markers) and indexed with SQLite FTS5, so Claude never re-reads PDFs. Questions cite their reference and location.
- **Grading:** Claude grades free-form answers against a per-question rubric (harder questions → more rubric points needed to pass), then maps the score to again/hard/good/easy deterministically. Rubric results are stored too.
- **Mastery levels L0–L3** on every question, defined by ability rather than content: L0 orientation (what/why, plain English), L1 working knowledge (concepts, qualitative trade-offs), L2 applied (formulas, worked calculations, using the tools), L3 expert (design decisions under constraints, diagnosis, critique). Topics carry a long-term target level. Subtopics = breadth, levels = depth. *How levels and nesting interact is expected to need tuning — treat as provisional.*
- **Main program vs. projects:** one topic graph and one review log for everything. Each topic/question is either in the **main program** (high bar: "matters to me in 10 years") or **project-scoped**. Projects link to existing general topics (optionally requiring a deeper level) and can own project-only topics. When a project ends, its items are promoted into the main program or archived (unscheduled, history kept).
- **Two study skills:** `/study` = daily main-program session only (drives home-page counts and streak). `/study-project <name>` = one project's questions, on demand. Project scheduling is gentler (lower desired retention / capped intervals).
- **Question usefulness ratings:** any question can be rated 1–5 for usefulness, with optional free-text feedback on why. Optional and low-friction during a session (never forced on every question).
- **Refine skill:** a skill that reads usefulness ratings + feedback and updates the question-writing criteria for each level (L0–L3), including curated good and bad examples. The question-generation skill uses these criteria, so generation improves over time. Claude classifies each piece of feedback from the user's words: *badly made question* (wording, triviality, wrong level) → updates how questions are written and rewrites/retires that question; *knowledge not worth having* → updates what gets chosen for questions and may drop it from the main program. Same pattern as distilling PR-comment history into a review skill.
- **Scheduler:** FSRS (`fsrs-rs`). Daily cap on new main-program questions; reviews are never capped.
- **Projects page** UI is a separate, later plan; the data model above should leave room for it.

## Open decisions

- **Claude ↔ database interface:** MCP server vs. CLI + skill; exact tools exposed.
- **Detailed schema** for topics, cross-links, questions, rubrics, references, reviews.

## Design foundations

- [README.md](README.md) — core principles of *Ultralearning*, *Make It Stick*, and *The Procrastination Equation*, with what each means for the app. Check design choices against these.
- [docs/research/spaced-repetition.md](docs/research/spaced-repetition.md) — SRS/Anki/FSRS lessons and their implications. Consult before changing scheduling, grading, workload, or question-writing behavior.
- **Topic schema details** (see below).

## Product outline

- **Home:** navbar (Map, Topics, Projects); today summary (answered today, remaining today, current streak); GitHub-style activity heatmap whose day tooltips show question count, grade breakdown, and topics studied; remaining-questions section.
- **Map:** zoomable visual map of topics. Size = number of questions; color = answer strength (red weak → blue strong); saturation = recency (faded if not studied lately). Click to zoom into subtopics, breadcrumbs to go back, down to individual questions. A question opens its own page with question text and attempt history. Time controls step back/forward one day and a play button animates the whole history over a few seconds.
- **Topics:** a list of topics.
- **Projects:** later, separate plan.
- **Stretch:** GPU-accelerated 3D graph of topics and their cross-links, colored by strength, with tooltips and navigation between neighboring topics.

## Design principles

- **Topics nest and cross-link.** Topics form a hierarchy, and topics/questions can be linked to others with a strength ("how related"). Model links as weighted edges so the 2D map and a future 3D graph read the same data.
- **Review history is an append-only log.** Every grade is a stored event; anything shown for a past date (map playback, heatmap) is derived from the log, never from overwritten state.
- **Keep rendering separate from data.** The backend serves "knowledge state" (per topic/question strength, recency, size, links) as plain data, optionally as of a date. Views — 2D map, 3D graph — are clients of that.

## Git

- Commits are authored by the user only. Do **not** add `Co-Authored-By: Claude` (or any Claude attribution) to commit messages or PR descriptions.
