# Learning Tool

A personal spaced-repetition learning tool (Anki-like). Claude asks the user questions, the user's answers are graded (again / hard / good / easy), and a scheduler decides when each question comes back. A local web app shows progress and a map of what the user knows.

**Status: milestones 1, 3 and 4 implemented** (study loop, topic building, web app part one, the knowledge map; projects and 3D are next). Design docs go in `docs/superpowers/specs/`, implementation plans in `docs/superpowers/plans/`. Update this file when decisions are made — don't let it drift from reality.

## Layout

- `crates/core/` (`learning-core`): all logic — `db` (open/migrate/seed), `settings`, `clock` (study day), `grading`, `scheduler` (FSRS), `topics`, `questions`, `queue`, `session`, `stats`, `status_log` (append-only status changes), `timeline` (per-day map history, computed on request), `chunking`, `references`, `guide`, `backup`. Migrations in `crates/core/migrations/`, first guide in `crates/core/seed/guide-v1.md`.
- `crates/cli/` (`learning-tool`, a lib + binary): clap CLI; `mcp` serves the core over stdio via `rmcp` (`src/mcp/`); `serve` runs the axum REST API plus the embedded React app (`src/web/`, assets via `rust-embed` from `web/dist`).
- `web/`: the React app (Vite, TypeScript, TanStack Router + Query, shadcn/Tailwind). API types in `web/src/api/generated/` come from Rust via `ts-rs`: never edit them by hand. Pure map logic (tree, circle-packing layout, OKLCH colors, staleness) is in `web/src/lib/map/`; the map components are in `web/src/components/map/`.
- `skills/`: `/study`, `/build-topic`, `/refine-questions` (symlinked into `~/.claude/skills/` by `scripts/install.sh`).

## Commands

- `cargo test --workspace` — all Rust tests (core unit tests use in-memory SQLite; `crates/cli/tests/mcp_stdio.rs` drives the MCP server, `web_api.rs` the HTTP API in-process).
- `cargo test --features learning-core/ts,learning-tool/ts export_bindings` — regenerate `web/src/api/generated/` after changing an API type; commit the result.
- `pnpm --dir web test`, `pnpm --dir web check` (Biome), `pnpm --dir web build`.
- Frontend dev loop: `learning-tool serve --no-open` in one terminal, `pnpm --dir web dev` in another (Vite proxies `/api` to port 4747).
- `./scripts/install.sh` — install binary, register MCP server, link skills.
- After rebuilding the binary, restart the MCP server in Claude Code (`/mcp`) to pick it up.

## Conventions

- Core functions take `now: DateTime<Utc>` explicitly — no hidden clock.
- SQL uses sqlx runtime queries (`query`/`query_as`) with static strings; every query is covered by tests. sqlx 0.9 rejects `format!`-built SQL — bind values instead (use `json_each(?)` for lists).
- The repo may sit on exFAT, where macOS writes `._*` sidecar files: `crates/core/build.rs` strips them from `migrations/`, `rust-embed`, tsconfig, Biome, Vitest and the router plugin ignore them, and `web/.npmrc` uses a flat, copied `node_modules` (no symlinks on exFAT).

## Decided

- **Backend:** Rust.
- **Storage:** a local SQLite database. Single user, runs on the user's machine.
- **Frontend:** React + TypeScript (Vite, TanStack, shadcn). The goal is a tool to actually use, so pick the mature ecosystem. API types are generated from the Rust structs (e.g. `ts-rs` / `specta`) rather than hand-written. The 3D stretch goal may be a Rust/WASM module embedded in the React app.
- **Quizzing happens in Claude Code**, not in the web app. The web app is for tracking, visualization, and project features.
- **Claude ↔ database: an MCP server** (`learning-tool mcp`, stdio) exposing typed tools, plus a small admin CLI (`migrate`, `stats`, `backup`, later `serve`). Skills hold the workflows; Rust holds data and scheduling.
- **Claude writes questions with the user.** A "build a topic" skill: Claude finds references (PDFs, articles, textbooks, best-practice guides), stores them, extracts their essence, and generates questions. New questions go straight into the main program by default; reviewing them first is opt-in.
- **References:** metadata in SQLite; original files in a folder beside the DB; text extracted once to a text/Markdown file (with page markers) and indexed with SQLite FTS5, so Claude never re-reads PDFs. Questions cite their reference and location.
- **Grading:** Claude grades free-form answers against a per-question rubric (harder questions → more rubric points needed to pass), then maps the score to again/hard/good/easy deterministically. Rubric results are stored too.
- **Mastery levels L0–L3** on every question, defined by ability rather than content: L0 orientation (what/why, plain English), L1 working knowledge (concepts, qualitative trade-offs), L2 applied (formulas, worked calculations, using the tools), L3 expert (design decisions under constraints, diagnosis, critique). Topics carry a long-term target level. Subtopics = breadth, levels = depth. *How levels and nesting interact is expected to need tuning — treat as provisional.*
- **Main program vs. projects:** one topic graph and one review log for everything. Each topic/question is either in the **main program** (high bar: "matters to me in 10 years") or **project-scoped**. Projects link to existing general topics (optionally requiring a deeper level) and can own project-only topics. When a project ends, its items are promoted into the main program or archived (unscheduled, history kept).
- **Two study skills:** `/study` = daily main-program session only (drives home-page counts and streak). `/study-project <name>` = one project's questions, on demand. Project scheduling is gentler (lower desired retention / capped intervals).
- **Question usefulness ratings:** any question can be rated 1–5 for usefulness, with optional free-text feedback on why. Optional and low-friction during a session (never forced on every question).
- **Refine skill:** a skill that reads usefulness ratings + feedback and updates the question-writing criteria for each level (L0–L3), including curated good and bad examples. The question-generation skill uses these criteria, so generation improves over time. Claude classifies each piece of feedback from the user's words: *badly made question* (wording, triviality, wrong level) → updates how questions are written and rewrites/retires that question; *knowledge not worth having* → updates what gets chosen for questions and may drop it from the main program. Same pattern as distilling PR-comment history into a review skill.
- **Scheduler:** FSRS (`fsrs-rs`). Daily cap on new main-program questions; reviews are never capped.
- **Session sizing follows SRS best practice** (counts, not time; target feel ~15–20 min, ~10 questions):
  - Cap **new** main-program questions per day — default **3** (configurable). Never cap reviews; all due reviews are served.
  - When due reviews run high (default > 20), new questions pause automatically until the load drops.
  - After a gap, an overdue backlog is spread over several days, lowest retrievability first, rather than dumped into one session.
- **Adding questions to a topic:** prioritize the questions that matter most, broad before specific.
- **Milestone 1 = core study loop + topic building** (Rust core, schema, FSRS, Claude Code interface, `/study`, references, build-a-topic). No web app yet — the minimum needed to test whether the idea works.
- **Projects page** UI is a separate, later plan; the data model above should leave room for it.

## Dependencies (chosen)

- **Backend:** Cargo workspace — `core` lib (DB, scheduler, topic graph) + one `learning-tool` binary with subcommands (`mcp`, `serve`, `migrate`). `tokio`, `rmcp` (stdio transport), `sqlx` (SQLite, migrations, FTS5 for references; runtime queries — see Conventions), `fsrs`, `axum` + `tower-http` (web API, milestone 3), `ts-rs` (TS types from Rust), `chrono`, `clap`, `serde`/`serde_json`, `thiserror` (core) / `anyhow` (binary), `tracing` (logs to stderr/file — stdout belongs to MCP).
- **No Rust PDF crates:** skills run `pdftotext` (poppler) / `ocrmypdf`; Rust stores and indexes the extracted text.
- **Frontend (milestone 3+):** React 19, TypeScript, Vite, pnpm, TanStack Router + Query, shadcn/ui (Radix, Tailwind 4), `lucide-react`, `d3-hierarchy` for the map layout (colors are hand-computed OKLCH), `motion` for transitions, hand-rolled SVG heatmap with shadcn Tooltip, `date-fns`, Biome, Vitest + Testing Library.

## Milestone docs

- Milestone 1 design (schema, MCP tools, skills): `docs/superpowers/specs/2026-10-23-milestone-1-study-loop-design.md`; plan: `docs/superpowers/plans/2026-10-23-milestone-1-study-loop.md`.
- Milestone 4 design (the knowledge map): `docs/superpowers/specs/2026-10-24-milestone-4-map-design.md`; plan: `docs/superpowers/plans/2026-10-24-milestone-4-map.md`.

## Future ideas

- [docs/ideas/model-agnostic-clients.md](docs/ideas/model-agnostic-clients.md): using the study workflows from other agent tools (OpenCode, other LLM providers).

## Design foundations

- [README.md](README.md) — core principles of *Ultralearning*, *Make It Stick*, and *The Procrastination Equation*, with what each means for the app. Check design choices against these.
- [docs/research/spaced-repetition.md](docs/research/spaced-repetition.md) — SRS/Anki/FSRS lessons and their implications. Consult before changing scheduling, grading, workload, or question-writing behavior.

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

## Reviews

After building new work, run the relevant architecture reviews on the diff: `/review-backend`, `/review-database`, `/review-frontend` and `/review-code`. Fix the high findings and normally the medium ones in code you just wrote, then report what you left. See [docs/review-skills.md](docs/review-skills.md). Bug hunting is separate (`/code-review`).

## Git

- Commits are authored by the user only. Do **not** add `Co-Authored-By: Claude` (or any Claude attribution) to commit messages or PR descriptions.
