# Milestone 3: Web app, part one

Date: 2026-10-24. Status: approved 2026-10-24.

## Intent

Robin studies in Claude Code (`/study`). The web app is where Robin **sees progress** and **tends the knowledge base**:
- the day's numbers, the streak and a year of activity
- browsing topics at any depth
- reading a question with its full attempt history
- small edits: rename or archive topics, add subtopics, reword questions, rate questions, and **pin** questions for the next study session

It runs on demand on localhost. It is built so that deploying it as an ordinary web app later is straightforward, but local use is the priority.

The map, projects and 3D graph are later milestones. This milestone lays the foundations they build on: the HTTP API, the daily-activity data, and the frontend stack.

Builds on: [milestone 1 design](2026-10-23-milestone-1-study-loop-design.md), CLAUDE.md, the README principles, and the review skills in `.claude/skills/`.

## Goals

1. `learning-tool serve` starts a local web server and opens the browser. One binary serves the API and the React app.
2. **Home page:**
   - a navbar: Home, Topics, then Map and Projects shown greyed out
   - three tiles: answered today with its grade breakdown, remaining today, and the streak with the best streak
   - a GitHub-style heatmap for the last 12 months; each day's tooltip shows the question count, grade breakdown and topics studied
   - a *Coming up* panel: tomorrow's load, new questions waiting with their days of capacity, and pinned questions
   - a *Needs attention* panel: leeches, and ratings not yet used by `/refine-questions`
3. **Topic pages, at any depth:**
   - clickable breadcrumbs
   - Rename, Archive/Unarchive and "+ Subtopic" actions
   - subtopic cards showing counts by level, how many are studied, average recall and a progress bar
   - the topic's own questions, with a "show all below" toggle
   - links and references
   - `/topics` is the root, and its subtopics are the top-level topics
4. **Question page:**
   - the prompt, answer, rubric (essential points marked), source and level
   - a schedule box: due date, reviews, lapses and recall now
   - pin/unpin, edit and suspend/unsuspend
   - a rating of 1–5 with optional feedback
   - the **attempt history**: every review across all revisions, with date, the wording asked, Robin's answer, rubric points hit or missed with notes, score, grade and Claude's feedback
5. **Pins**, as a core feature used by both the web app and `/study`, following the rules below.
6. Every number the web app shows comes from the same `learning-core` functions `/study` uses, so the two can never disagree.

## Non-goals

- The map, zoom, coloring, history playback, projects and 3D (milestones 4–6).
- Authentication, multiple users and deployment work. This milestone leaves seams for them but builds none.
- Taking quizzes in the browser. Quizzing stays in Claude Code.
- Creating questions in the web app. `/build-topic` does that; the web app only edits existing ones.
- Mobile-first design. The layout has to work in a normal desktop browser window, but tablet and phone polish can wait.

## Anti-goals

- **The web app and `/study` disagree** about any count, the streak or what's due.
- **An edit loses history.** Rewording must create a revision; archiving must keep everything; nothing is hard-deleted.
- **A blank page after install**, because the frontend wasn't built or found.
- **Answers leak by accident.** The question page shows the answer. That's fine, since Robin is browsing on purpose, but the topic list and the home page must not.
- **Slow or janky pages** on a single-user local database. Every page should feel instant.

## Approaches considered

- **A. A REST API with the React app embedded in the binary. Chosen.**
  - Its resource-shaped endpoints fit TanStack Query (caching and invalidation) and are the standard shape for deploying later.
  - The single-binary install is kept.
- **B. Reusing the MCP tools over a generic `POST /api/call/<tool>`.** Rejected:
  - the tools are shaped for Claude, with text summaries and session-oriented calls
  - everything would be a POST, which makes caching awkward
  - it's an odd public API
- **C. Like A, but serving the frontend from files on disk.** Rejected: the install would ship and locate a folder of files alongside the binary, which risks the blank-page anti-goal.

## Architecture

```
crates/core/            learning-core: + pins, activity, overview, API-facing view structs (ts-rs)
crates/cli/
  src/main.rs           + `serve` subcommand
  src/web/mod.rs        router, static assets, error mapping, middleware seam
  src/web/routes/*.rs   thin handlers calling learning-core
  build.rs              ensures web/dist exists (placeholder page if the frontend isn't built)
web/                    Vite + React + TypeScript app (pnpm)
  src/api/generated/    ts-rs output (committed)
  src/routes/           TanStack Router routes
  src/components/       UI components (shadcn primitives in components/ui)
```

### Serving

- **Command:** `learning-tool serve [--port N] [--host H] [--no-open]`.
  - It binds `127.0.0.1` by default; `--host` exists for later deployment.
  - The port comes from a new setting `web_port` (default 4747). A port already in use gives a clear error that suggests `--port`.
  - Unless `--no-open` is given, it opens the default browser at the root URL (via the `open` crate) and logs the URL to stderr.
- **Routes:**
  - `/api/*` returns JSON.
  - Every other path serves the embedded `web/dist` files. Unknown non-API paths fall back to `index.html`, so client-side routes work on reload.
- **Embedding:**
  - `rust-embed` compiles `web/dist` into the release binary; debug builds read from disk.
  - `crates/cli/build.rs` writes a placeholder `web/dist/index.html` ("Frontend not built: run `pnpm --dir web build`") if `web/dist` is missing, so the Rust build never fails on it.
  - `scripts/install.sh` builds the frontend first.
- **Middleware:**
  - `tower-http` request tracing to stderr and the log file
  - a request body size limit
  - one no-op layer marked as the auth seam
  - no CORS, since everything is same-origin
- **Errors:** JSON `{ "error": { "kind": "invalid"|"not_found"|"conflict"|"internal", "message": "…" } }`, with HTTP status 400/404/409/500. These map from the core `Error` variants, the same way `respond` does for MCP.
- **Concurrency:** writes go through the core functions, which already use `begin_write`, so a browser tab and a `/study` session are safe together.

### Types shared with TypeScript

- API-facing structs derive `ts_rs::TS` behind a `ts` cargo feature on `learning-core` (and the CLI's request structs).
- A test target (`cargo test --features ts export_bindings`) writes them to `web/src/api/generated/`.
- The generated files are committed, so the frontend builds without Rust. CI or a review checks that they're current: FE-R4 already says never hand-write them.

## Core additions (`learning-core`)

### Pins

**Migration `0003_pins.sql`:**
- a new table `pins(id, question_id, pinned_at, unpinned_at NULL, used_in_session_id NULL, used_at NULL, created_at, updated_at)`
- a unique partial index giving at most one *active* pin per question, where active means `unpinned_at IS NULL AND used_in_session_id IS NULL`
- `ALTER TABLE session_items ADD COLUMN pin_id INTEGER REFERENCES pins(id)`

**Rules, as agreed:**
- Pinned questions come **first** in the next session that is planned, in pin order, ahead of the warm-up.
- How a pin interacts with the cap:
  - A pin that's **due anyway** costs nothing extra; it just moves to the front.
  - A pin that's **new** or **not yet due** uses one of today's new-question slots: `new_allowed` is reduced by one for each such pin.
- If pins exceed the remaining slots (including when new questions are paused by the review threshold), the extras **stay pinned** for a later session.
- A session that's already planned and open isn't changed. Pins join the next planning, which includes the re-plan that `start` does when an open session has no unserved items left.
- A pin is **used** when a review is recorded for its session item: `used_in_session_id` and `used_at` are set. A skipped pinned item stays pinned.
- **Unpin** sets `unpinned_at`. Pin rows are never deleted, so the history of what was pinned stays.
- Pinning a retired or suspended question is rejected (`Invalid`). Pinning an already pinned question is a no-op that returns the existing pin.

**API:**
- `pins::pin(pool, question_id, now)`, `pins::unpin(pool, question_id, now)`, and `pins::active(pool) -> Vec<PinnedItem>`.
- `queue::plan` gains the pinned candidates. `queue::load_candidates` returns them with their due state.
- `NextQuestion` gains `pinned: bool`.

**MCP and the skill:**
- new tools `pin_question` and `unpin_question`
- `/study` says "you pinned this" when `pinned` is true, and Robin can say "pin that one" mid-session

### Activity and overview

- **`stats::activity(pool, s, from: NaiveDate, to: NaiveDate) -> Vec<DayActivity>`:**
  - `DayActivity { study_day, count, by_grade: [u32; 4], topics: Vec<TopicRef> }`
  - main-program sessions only
  - only days that have reviews are returned; the frontend fills the gaps
  - milestone 4 history playback will reuse this
- **`stats::best_streak(pool, s) -> u32`:** the longest run of qualifying days.
- **`stats::overview(pool, s, now) -> HomeSummary`:** one call for the home page. It holds:
  - `TodayStats`
  - `by_grade_today`
  - `best_streak`
  - `new_waiting` and `new_waiting_days`, at the current cap
  - `pinned: Vec<PinnedItem>` (question id, prompt, topic path)
  - `leeches: Vec<QuestionRef>`
  - `ratings_unprocessed: u32`
- **`TopicNode` / `TopicCard` gains `studied: u32`:** the number of active questions in the subtree with at least one review.

### Page view structs

These are built in core, so handlers stay thin and the shapes are unit-tested:

- **`TopicPage`:** the topic, breadcrumbs as `Vec<TopicRef>`, children as `Vec<TopicCard>`, questions as `Vec<QuestionRow>` (direct, or the whole subtree when asked), links resolved to names, and references (id, title, kind).
  - `QuestionRow` holds: id, level, prompt, status, `due_on`, pinned, recall now, reviews count.
  - `QuestionRow` **never includes the answer or rubric** (see the anti-goals).
- **`QuestionPage`:** the existing `QuestionDetail` plus `pinned`, `recall_now`, the source citation, and `newest_revision_id` (set when this question is retired because it was reworded).

## HTTP API

| Method & path | Core call | Notes |
|---|---|---|
| `GET /api/home` | `stats::overview` | Home tiles and panels |
| `GET /api/activity?from&to` | `stats::activity` | Heatmap; defaults to the last 365 days |
| `GET /api/topics` | root `TopicPage` | Top-level topics as cards |
| `GET /api/topics/:id?all=true` | `TopicPage` | `all` lists the whole subtree's questions |
| `POST /api/topics` | `topics::create` | "+ Subtopic" (`parent_id`, `name`, `target_level`, `description`) |
| `PATCH /api/topics/:id` | `topics::update` | Rename, describe, re-target, archive/unarchive |
| `GET /api/questions/:id` | `QuestionPage` | Includes the attempt history |
| `PATCH /api/questions/:id` | `questions::update` | Rewording returns the **new revision**; the client navigates to its URL |
| `POST /api/questions/:id/status` | `questions::set_status` | `active` or `suspended` only (no retiring from the web app) |
| `POST /api/questions/:id/ratings` | `guide::rate` | `usefulness` 1–5 and `feedback`; no classification |
| `PUT /api/questions/:id/pin` | `pins::pin` | Idempotent |
| `DELETE /api/questions/:id/pin` | `pins::unpin` | Soft: sets `unpinned_at` |

## Frontend

**Stack** (from CLAUDE.md): React 19, TypeScript (strict), Vite, pnpm, TanStack Router and TanStack Query, shadcn/ui on Tailwind 4, `lucide-react`, `date-fns`, Biome, and Vitest with Testing Library.

**Routes:**
- `/`: Home
- `/topics`: the root topic page
- `/topics/$topicId`: a topic page
- `/questions/$questionId`: a question page
- Unknown routes get a not-found page.

**Home:**
- three tiles
- the **heatmap**:
  - hand-rolled SVG: 53 weeks × 7 days
  - color steps from the count
  - today outlined
  - a shadcn Tooltip per day with the count, grade breakdown and topics
  - days are keyboard-focusable, so tooltips open on focus too (FE-G7)
- *Coming up* and *Needs attention* panels linking to questions

**Topic page:**
- breadcrumbs
- a header with Rename (dialog), Archive/Unarchive (confirmation dialog) and "+ Subtopic" (dialog with name, target level and description)
- subtopic cards
- a question table with a pin toggle per row and a "show all below" toggle
- links and references
- a "show archived" toggle on subtopics

**Question page:**
- a header with the topic path and level, and Pin, Edit and Suspend buttons
- answer, rubric and source
- the schedule box
- the rating box: 5 stars, optional feedback, submit; past ratings listed
- the attempt history table, whose rows expand to show per-point notes and feedback
- **Retired questions** show a banner linking to the newest revision.

**Edit dialog:**
- prompt, answer, level and pass threshold fields
- a **rubric editor**: add, remove and reorder points; text; weight; an essential toggle
- validation that mirrors the core: at least one point, unique non-empty ids, weight > 0, threshold in (0, 1]
- Saving warns that rewording creates a new revision and keeps the history.

**State and design:**
- All server state goes through TanStack Query. Mutations invalidate the affected queries: home, the topic page and the question page.
- Loading, error and empty states everywhere (FE-G8).
- Light and dark mode via design tokens.

## Calls I made (not discussed; check these)

- **Port and browser:** the default port is 4747, set by a `web_port` setting or `--port`. The browser opens automatically unless `--no-open` is given.
- **Pins:** used when the pinned item is *answered*, not merely planned, so a skipped pin stays pinned.
- **Status changes from the web:** only suspend and unsuspend. Retiring stays with Claude (`/refine-questions`), because it's the more consequential action.
- **New dependencies:** `axum`, `tower-http` and `ts-rs` are already chosen. `rust-embed` (static embedding) and `open` (launching the browser) are new and small.
- **The home page is one `GET /api/home`**, not many small calls, so it loads in one request.
- **The heatmap and streak count main-program sessions only**, as decided for `/study`.
- **The topic list doesn't show answers or rubrics.** Only the question page does.
- **"Best streak" is a new number** on the streak tile.

## Testing and verification

- **Core unit tests:**
  - pins: due / new / not-due slot accounting, overflow stays pinned, an open session isn't changed, used-on-record, skipped stays pinned, unpin keeps the row, retired questions are rejected
  - activity: grades, topics, main-only, the date range
  - best streak
  - overview counts
  - `TopicPage` and `QuestionRow` never contain the answer or rubric
  - `QuestionPage` sets `newest_revision_id`
- **API tests:** the axum `Router` driven in-process (`tower::ServiceExt::oneshot`) against a temporary database, for every endpoint, including error statuses and the SPA fallback.
- **The MCP end-to-end test** extended with `pin_question` / `unpin_question`.
- **Frontend (Vitest + Testing Library):**
  - heatmap cells and tooltip content for a given activity list
  - rubric editor validation
  - the pin toggle calling the right mutation
  - breadcrumbs rendering a path
  - the not-found route
- **Manual acceptance:**
  1. `./scripts/install.sh`, then `learning-tool serve`. The home page shows today's real numbers, matching `learning-tool stats`.
  2. Click through Topics › Electric circuits › Impedance and open a question; the attempt history matches what was answered.
  3. Pin a new question, then start `/study` in Claude Code: it comes first, and the new-question count drops by one.
  4. Reword a question in the web app: the page moves to the new revision, and the history is intact.
- **Reviews:** after building, run `/review-backend`, `/review-database`, `/review-frontend` and `/review-code` on the branch (CLAUDE.md).

## Left to the builder

- Module and file layout inside `web/`, component boundaries and styling details within the design tokens.
- Exact DTO field names, as long as the generated types match.
- Whether `web/` routes are file-based or code-based in TanStack Router.
- The heatmap color steps and tooltip styling.
- The build script mechanism (`just` or shell) for "build web, then install".

## Provisional

- The home panels' contents (*Coming up*, *Needs attention*) should be revisited after using it for a while.
- Default port 4747.
