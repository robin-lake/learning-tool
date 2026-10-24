# Milestone 3: Web App (Part One) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pins and home-page data to `learning-core`, an axum REST API with the React app embedded in the `learning-tool` binary (`learning-tool serve`), and the Home, Topic and Question pages.

**Architecture:**
- **Core first.** New core modules come first: `pins`, `stats::{activity, best_streak, overview}`, and `views` (page-shaped structs). They're unit-tested against in-memory SQLite.
- **API.** `crates/cli` becomes a library plus a binary, so its `web` and `mcp` modules can be tested from `tests/`. `web::router` maps REST endpoints to core calls and serves `web/dist` through `rust-embed`.
- **Frontend.** `web/` is a Vite + React + TypeScript app: TanStack Router and Query, shadcn on Tailwind 4. It consumes types generated from Rust by `ts-rs`.

**Tech Stack:**
- **Rust:** `axum` 0.8, `tower-http` 0.7, `tower` 0.5 (dev, for `oneshot`), `http-body-util` (dev), `rust-embed` 8, `mime_guess`, `open` 5, and `ts-rs` 12 with `chrono-impl`.
- **Web:** React 19, TypeScript, Vite 8, `@vitejs/plugin-react`, `@tanstack/react-router` + `@tanstack/router-plugin`, `@tanstack/react-query`, Tailwind 4 (`@tailwindcss/vite`), shadcn/ui (Radix), `lucide-react`, `date-fns`, Biome 2, Vitest 5 + `@testing-library/react` + `jsdom`, all through pnpm.

**Spec:** `docs/superpowers/specs/2026-10-24-milestone-3-web-app-design.md`. Read it alongside this plan. Its Core additions, HTTP API and Frontend sections are normative.

## Global Constraints

- No `Co-Authored-By: Claude` or other Claude attribution in commits.
- Core functions take `now: DateTime<Utc>`; only binaries call `Utc::now()` (BE-R2).
- All domain logic lives in `learning-core`; web handlers only parse input, call core and serialize (BE-R1).
- Write transactions use `db::begin_write` (DB-R2).
- Nothing user-facing is hard-deleted. Unpinning sets `unpinned_at`; pin rows stay (DB-R4).
- Every SQL path has a test (DB-R9).
- API types come from `ts-rs` and are never hand-written (FE-R4). They're generated into `web/src/api/generated/` and committed.
- List views never carry answers or rubrics: `QuestionRow`, `TopicPage`, `HomeSummary` and `PinnedItem` must not contain `answer` or `rubric` fields.
- The server binds `127.0.0.1` by default; the default port is the `web_port` setting (4747).
- JSON errors use the shape `{"error":{"kind":"invalid|not_found|conflict|internal","message":"…"}}`, with status 400/404/409/500.
- The heatmap, streak and home counts cover main-program sessions only.
- Retiring a question isn't possible from the web app: the status endpoint accepts only `active` or `suspended`.

## Review Focus

1. **`learning-tool serve` when port 4747 is already taken.** Expected: exits with a message that names the port and suggests `--port`, not a panic. *Test: Task 6, `bind_error_mentions_port_flag`.*
2. **A pinned question whose topic is later archived.** Expected: it isn't served, it doesn't use a slot, and it isn't listed as pinned on the home page; the pin row stays. *Test: Task 1, `pin_on_archived_topic_is_ignored`.*
3. **A brand-new install with no history.** Expected: the heatmap renders an empty 53×7 grid and the home page shows zeros, with no errors. *Test: Task 8, `heatmap_renders_empty_year`.*
4. **The server is down or returns 500 while a page is open.** Expected: an error panel with a Retry button, not a blank page. *Test: Task 7, `query_error_shows_retry`.*
5. **Very long topic names or prompts in tables and cards.** Expected: truncated with the full text in a `title` attribute; the layout doesn't overflow. *Test: Task 9, `long_prompt_is_truncated_with_title`.*

---

## File Structure

```
.cargo/config.toml                         # [env] TS_RS_EXPORT_DIR = web/src/api/generated (relative)
crates/core/migrations/0003_pins.sql       # pins table, session_items.pin_id, web_port setting
crates/core/src/pins.rs                    # pin, unpin, active, is_pinned, load_pinned (candidates)
crates/core/src/queue.rs                   # plan() takes pinned candidates; PlannedItem
crates/core/src/session.rs                 # pin_id on items; mark used on record; NextQuestion.pinned
crates/core/src/stats.rs                   # activity, best_streak, overview (HomeSummary)
crates/core/src/views.rs                   # TopicRef, TopicCard, QuestionRow, TopicPage, QuestionPage
crates/core/src/types.rs                   # str_enum! derives TS when feature "ts"
crates/core/Cargo.toml                     # feature "ts" = ["dep:ts-rs"]
crates/cli/src/lib.rs                      # pub mod mcp; pub mod web;
crates/cli/src/main.rs                     # uses learning_tool::{mcp, web}; + Serve subcommand
crates/cli/src/web/mod.rs                  # AppState, router(), serve()
crates/cli/src/web/error.rs                # ApiError: From<learning_core::Error>, IntoResponse
crates/cli/src/web/assets.rs               # rust-embed WebDist + static/SPA fallback handler
crates/cli/src/web/routes.rs               # all /api handlers + request bodies
crates/cli/build.rs                        # placeholder web/dist/index.html when missing
crates/cli/tests/web_api.rs                # in-process API tests (oneshot)
web/                                       # Vite app (see Task 7)
scripts/install.sh                         # build web, then cargo install
skills/study/SKILL.md                      # pinned questions + "pin that one"
```

---

### Task 1: Pins (data and API in core)

**Files:**
- Create: `crates/core/migrations/0003_pins.sql`, `crates/core/src/pins.rs`
- Modify: `crates/core/src/lib.rs` (`pub mod pins;`)

**Interfaces:**
- Produces:
  - `pub struct Pin { pub id: i64, pub question_id: i64, pub pinned_at: DateTime<Utc>, pub unpinned_at: Option<DateTime<Utc>>, pub used_in_session_id: Option<i64>, pub used_at: Option<DateTime<Utc>> }`
  - `pub struct PinnedItem { pub pin_id: i64, pub question_id: i64, pub prompt: String, pub level: u8, pub topic_path: Vec<String>, pub pinned_at: DateTime<Utc> }`. No answer or rubric fields.
  - `pub async fn pin(pool, question_id: i64, now) -> Result<Pin>`:
    - returns `Invalid` unless the question is `active` (message names its status)
    - returns `NotFound` for a missing question
    - if an active pin already exists, it returns that pin and writes nothing
  - `pub async fn unpin(pool, question_id: i64, now) -> Result<()>`: sets `unpinned_at` on the active pin. With no active pin it is a no-op `Ok(())`. A missing question is `NotFound`.
  - `pub async fn is_pinned(pool, question_id) -> Result<bool>`
  - `pub async fn active(pool) -> Result<Vec<PinnedItem>>`: active pins on schedulable questions only (question `active`, `scope='main'`, topic and every ancestor `active`), ordered by `(pinned_at, id)`.
  - `pub(crate) async fn load_pinned(pool, today: NaiveDate, now) -> Result<Vec<PinnedCandidate>>`, with `pub struct PinnedCandidate { pub pin_id: i64, pub candidate: queue::Candidate, pub is_due: bool }`. Same filter and order as `active`; `is_due` is `due_on <= today`.
  - **Migration `0003_pins.sql`:**
    - `pins` with the design doc's columns plus `created_at`/`updated_at`
    - `CREATE UNIQUE INDEX pins_one_active ON pins(question_id) WHERE unpinned_at IS NULL AND used_in_session_id IS NULL`
    - `ALTER TABLE session_items ADD COLUMN pin_id INTEGER REFERENCES pins(id)`
    - `INSERT INTO settings VALUES ('web_port','4747')`
  - `Settings` gains `pub web_port: u16`, with `set` validating 1–65535.

- [ ] **Step 1: Write failing tests in `pins.rs`:**

```rust
#[tokio::test] async fn pin_is_idempotent_and_listed_in_order() {
    // pin q2 then q1 (different `now`), pin q2 again → same pin id; active() → [q2, q1]
}
#[tokio::test] async fn unpin_keeps_the_row() {
    // pin, unpin → active() empty; count(*) FROM pins == 1 with unpinned_at set; unpin again → Ok
}
#[tokio::test] async fn pin_rejects_non_active_and_missing() {
    // suspended question → Error::Invalid (message contains "suspended"); 999 → NotFound
}
#[tokio::test] async fn pin_on_archived_topic_is_ignored() {
    // pin q in topic T, archive T's parent → active() empty, load_pinned() empty, row still present
}
#[tokio::test] async fn repin_after_unpin_creates_new_row() { /* pin, unpin, pin → 2 rows, 1 active */ }
// settings.rs
#[tokio::test] async fn web_port_default_and_validation() { /* 4747; set "0" and "70000" → Invalid */ }
```

- [ ] **Step 2: Run `cargo test -p learning-core pins`.** Expected: FAIL (not defined).
- [ ] **Step 3: Implement** the migration, `pins.rs` and the `web_port` setting.
- [ ] **Step 4: Run `cargo test -p learning-core`.** Expected: PASS (all tests).
- [ ] **Step 5: Commit** with `git commit -m "Add question pins to the core"`.

---

### Task 2: Pins in the session queue

**Files:**
- Modify: `crates/core/src/queue.rs`, `crates/core/src/session.rs`, `crates/core/src/stats.rs` (`today` uses the new `plan` signature)

**Interfaces:**
- Consumes: `pins::load_pinned` and `PinnedCandidate` (Task 1).
- Produces:
  - `pub struct PlannedItem { pub question_id: i64, pub kind: ItemKind, pub pin_id: Option<i64> }`
  - `QueuePlan.items: Vec<PlannedItem>`, and `QueuePlan` gains `pub pinned_used: u32` and `pub pinned_waiting: u32`.
  - `pub fn plan(due: Vec<Candidate>, new: Vec<Candidate>, pinned: Vec<PinnedCandidate>, new_introduced_today: u32, s: &Settings) -> QueuePlan`. Steps, in order:
    1. Compute `slots = new_allowed` exactly as before (cap minus introduced, or 0 at the pause threshold).
    2. Walk `pinned` in order:
       - If `is_due`: take it, remove it from `due`, and give it kind `Review`.
       - Otherwise, if `slots > 0`: take it and do `slots -= 1`. Its kind is `New` if it has no state, else `Review`.
       - Otherwise skip it: it counts toward `pinned_waiting`.
    3. Remove taken pinned questions from `new`. Then plan the remaining due and new exactly as before, using `slots` as the new-question allowance. Keep the backlog limit, warm-up, spreading and de-clustering.
    4. Final order: the pinned items in pin order, followed by the existing plan. De-clustering applies only after the pinned prefix.
    5. `new_allowed` in the result is the number of scheduler-chosen new items plus `slots` used by pins.
  - `queue::load_candidates` also returns pinned candidates. Its signature becomes `-> Result<(Vec<Candidate>, Vec<Candidate>, Vec<PinnedCandidate>, u32)>`.
  - In session `start`: each `session_items` row is inserted with `pin_id`.
  - In `record_review`: if the item has a `pin_id`, run `UPDATE pins SET used_in_session_id = ?, used_at = ?, updated_at = ? WHERE id = ?` in the same transaction.
  - `NextQuestion` gains `pub pinned: bool`, which is true when the item has a `pin_id`.

- [ ] **Step 1: Write failing tests.** Pure `plan` tests in `queue.rs`:
  - `pinned_due_moves_first_without_using_a_slot`: 2 due, one pinned due. It's first; `new_allowed` is still 3.
  - `pinned_new_uses_a_new_slot`: cap 3, one pinned new. That pin is first, and only 2 scheduler-chosen new items follow.
  - `pinned_not_due_review_uses_a_slot`
  - `pins_beyond_slots_wait`: cap 3, 5 pinned new. 3 are taken and `pinned_waiting` is 2. They're taken in pin order.
  - `pins_wait_when_new_paused`: 20 due, one pinned new. It isn't taken; `pinned_waiting` is 1.
  - `pinned_prefix_not_declustered`: two pinned items from the same topic stay adjacent and first.

  DB and session tests in `session.rs`:
  - `pinned_item_served_first_and_flagged`
  - `pin_marked_used_on_record_not_on_plan`: after `start` the pin is still active; after `record_review` it's used.
  - `skipped_pin_stays_pinned`
  - `open_session_not_changed_by_new_pin`: start, pin something, then `start` again (resume). The new pin isn't in the session.
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement,** and update the existing `plan(...)` calls in tests to pass `vec![]` for pinned.
- [ ] **Step 4: Run `cargo test --workspace`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Serve pinned questions first, counting toward the new-question cap"`.

---

### Task 3: Pin tools for Claude, and the study skill

**Files:**
- Modify: `crates/cli/src/mcp/{params.rs,tools.rs}`, `crates/cli/tests/mcp_stdio.rs`, `skills/study/SKILL.md`

**Interfaces:**
- Consumes: `pins::{pin, unpin}` and `NextQuestion.pinned`.
- Produces:
  - `pin_question` and `unpin_question` tools, each taking `{question_id}`. The `next_question` summary says "(pinned by Robin)" when pinned.
  - In the skill:
    - When `pinned` is true, say briefly that this is a question Robin pinned.
    - "pin that one" / "pin this" calls `pin_question` for the current question; it'll come first next session.
    - "unpin …" calls `unpin_question`.

- [ ] **Step 1:** Extend `mcp_stdio.rs`: `TOOLS` has 29 names. Pin question 2 before `start_session`. The first `next_question` returns question 2 with `pinned: true`.
- [ ] **Step 2: Run `cargo test -p learning-tool --test mcp_stdio`.** Expected: FAIL.
- [ ] **Step 3: Implement the tools** and edit the skill (one short "Pins" section, plus a line in the flow).
- [ ] **Step 4: Run `cargo test --workspace`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add pin_question and unpin_question for Claude"`.

---

### Task 4: Activity, best streak and home overview

**Files:**
- Modify: `crates/core/src/stats.rs`, `crates/core/src/topics.rs` (`TopicNode.studied`)

**Interfaces:**
- Produces:
  - `pub struct TopicRef { pub id: i64, pub name: String }`. Defined in `views.rs` (Task 5), so create `views.rs` now with just this struct.
  - `pub struct DayActivity { pub study_day: NaiveDate, pub count: u32, pub by_grade: [u32; 4], pub topics: Vec<TopicRef> }`
  - `pub async fn activity(pool, from: NaiveDate, to: NaiveDate) -> Result<Vec<DayActivity>>`:
    - reviews in `kind='main'` sessions with `study_day` between `from` and `to` inclusive
    - one entry per day that has reviews, ascending
    - `topics` are the distinct primary topics of the reviewed questions, ordered by name
  - `pub async fn best_streak(pool, s) -> Result<u32>`: the longest run of consecutive qualifying days (same qualifying rule as `streak`).
  - `pub struct QuestionRef { pub id: i64, pub prompt: String, pub topic_path: Vec<String> }`
  - `pub struct HomeSummary { pub today: TodayStats, pub by_grade_today: [u32; 4], pub best_streak: u32, pub new_waiting: u32, pub new_waiting_days: u32, pub pinned: Vec<PinnedItem>, pub leeches: Vec<QuestionRef>, pub ratings_unprocessed: u32 }`
  - `pub async fn overview(pool, s, now) -> Result<HomeSummary>`:
    - `new_waiting` is the number of new candidates (main, live, active, no state)
    - `new_waiting_days = ceil(new_waiting / new_per_day)`
    - `leeches` are active main questions with `lapses >= leech_lapses`
  - `TopicNode` gains `pub studied: u32`: the number of active questions in the subtree with at least one review.

- [ ] **Step 1: Write failing tests:**
  - `activity_groups_by_day_with_grades_and_topics`: 2 days and 3 reviews across 2 topics. Check the counts, the `by_grade` arrays, and topic names ordered by name.
  - `activity_ignores_project_sessions_and_range`
  - `best_streak_finds_longest_run`: days 1–3, then 5–6. The answer is 3.
  - `overview_counts`: covers `by_grade_today`, `new_waiting` (and the days rounded up), and the pinned list, with no answer field (assert by serializing to JSON and checking there's no `"answer"` key), leeches and `ratings_unprocessed`.
  - `tree_counts_studied_questions`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `cargo test -p learning-core`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add daily activity, best streak and home overview"`.

---

### Task 5: Page views

**Files:**
- Modify: `crates/core/src/views.rs`, `crates/core/src/lib.rs`

**Interfaces:**
- Consumes: `topics::{tree, get, find}`, `questions::{get, find}`, `pins::is_pinned`, `scheduler::QState::retrievability`, `TopicRef` and `QuestionRef`.
- Produces:
  - `pub struct TopicCard { pub id, pub name, pub target_level: u8, pub status: TopicStatus, pub questions_by_level: [u32; 4], pub active_questions: u32, pub studied: u32, pub avg_retrievability: Option<f32> }`
  - `pub struct QuestionRow { pub id, pub topic_id, pub level: u8, pub prompt: String, pub status: QuestionStatus, pub due_on: Option<NaiveDate>, pub pinned: bool, pub recall_now: Option<f32>, pub reviews: u32 }`
  - `pub struct LinkView { pub topic: TopicRef, pub kind: LinkKind, pub weight: f32, pub outgoing: bool }`
  - `pub struct ReferenceRef { pub id: i64, pub title: String, pub kind: SourceKind }`
  - `pub struct TopicPage { pub topic: Option<Topic> /* None = root */, pub breadcrumbs: Vec<TopicRef>, pub children: Vec<TopicCard>, pub questions: Vec<QuestionRow>, pub links: Vec<LinkView>, pub references: Vec<ReferenceRef> }`
  - `pub async fn topic_page(pool, id: Option<i64>, all: bool, include_archived: bool, now) -> Result<TopicPage>`:
    - **Root** (`None`): children are the top-level topics; no questions.
    - **Otherwise:**
      - `questions` covers the topic's own non-retired questions, or with `all` the whole subtree's, ordered by `(level, priority, id)`.
      - `children` excludes archived topics unless `include_archived`.
  - `pub struct QuestionPage { pub detail: QuestionDetail, pub topic_refs: Vec<TopicRef>, pub pinned: bool, pub recall_now: Option<f32>, pub source_citation: Option<String>, pub newest_revision_id: Option<i64> }`
  - `pub async fn question_page(pool, id, now) -> Result<QuestionPage>`. `newest_revision_id` follows `revision_of` children forward to the newest. It is `None` when this question is itself the newest.

- [ ] **Step 1: Write failing tests:**
  - `root_page_lists_top_level_cards`
  - `topic_page_breadcrumbs_children_and_direct_questions`
  - `topic_page_all_includes_subtree`
  - `archived_children_hidden_unless_requested`
  - `rows_never_include_answer_or_rubric`: serialize a `TopicPage` and assert the JSON has no `"answer"` or `"rubric"` keys.
  - `links_resolved_with_direction`
  - `question_page_newest_revision_followed` (v1 → v2 → v3; the page for v1 gives v3)
  - `question_page_recall_and_pin`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `cargo test -p learning-core`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add topic and question page views"`.

---

### Task 6: Web server and API

**Files:**
- Create: `crates/cli/src/lib.rs`, `crates/cli/src/web/{mod.rs,error.rs,assets.rs,routes.rs}`, `crates/cli/build.rs`, `crates/cli/tests/web_api.rs`
- Modify: `crates/cli/src/main.rs`, `crates/cli/Cargo.toml`, the workspace `Cargo.toml`

**Interfaces:**
- Consumes: the core functions from Tasks 1–5, plus `topics::{create, update}`, `questions::{update, set_status}` and `guide::rate`.
- Produces:
  - `pub struct AppState { pub pool: SqlitePool, pub dirs: DataDir }` (Clone)
  - `pub fn router(state: AppState) -> axum::Router`: the routes from the design doc's HTTP API table, with `/api/*` returning a JSON 404 for unknown API paths and everything else going to `assets::static_handler`.
  - `pub async fn serve(pool, dirs, host: IpAddr, port: u16, open_browser: bool) -> anyhow::Result<()>`. A bind failure returns an error containing `port {port} is in use` and `--port`.
  - `pub struct ApiError(learning_core::Error)`, with `impl IntoResponse`: `Invalid`→400, `NotFound`→404, `Conflict`→409, and everything else 500 with a generic message (log the detail).
  - **Request bodies** (serde + `ts-rs`, in `routes.rs`):
    - `CreateTopicBody { parent_id: Option<i64>, name, target_level: u8, description: Option<String> }`
    - `UpdateTopicBody { name, description, target_level, status: Option<TopicStatus> }`, all optional
    - `QuestionPatch` reused from core
    - `StatusBody { status: QuestionStatus }`, which rejects anything other than active or suspended with 400
    - `RatingBody { usefulness: u8, feedback: Option<String> }`
    - `ActivityQuery { from: Option<NaiveDate>, to: Option<NaiveDate> }`, defaulting to the 365 days ending today
    - `TopicQuery { all: bool, archived: bool }`
  - **`assets.rs`:** `#[derive(RustEmbed)] #[folder = "$CARGO_MANIFEST_DIR/../../web/dist"] struct WebDist;`. It serves the exact file with a `mime_guess` content type. Unknown non-API paths get `index.html`.
  - **`build.rs`:** if `../../web/dist/index.html` is missing, it creates `web/dist/index.html` with the placeholder text from the spec. It also prints `cargo:rerun-if-changed=../../web/dist`.
  - **`main.rs`:** `Command::Serve { port: Option<u16>, host: Option<IpAddr>, no_open: bool }`. The port defaults to `Settings.web_port` and the host to `127.0.0.1`.
  - **Middleware:** `TraceLayer`, `RequestBodyLimitLayer` (1 MiB), and an `auth_seam` `middleware::from_fn` that passes requests through (with a comment saying it's the place to add auth).
  - **Dependencies:** add `axum`, `tower-http` (features `trace`, `limit`), `rust-embed`, `mime_guess`, `open`. Dev: `tower` (`util`), `http-body-util`.

- [ ] **Step 1: Write failing tests in `tests/web_api.rs`.** They build `router(AppState)` on a temporary-directory database seeded through core calls, then drive it with `oneshot`:
  - `home_returns_summary`
  - `activity_defaults_to_last_year`
  - `root_topics_lists_cards`
  - `topic_page_with_all_flag`
  - `create_and_rename_and_archive_topic`
  - `question_page_includes_history`
  - `rewording_returns_new_revision`
  - `status_rejects_retired_with_400`
  - `rating_validates_range_400`
  - `pin_put_and_delete_are_idempotent`
  - `unknown_api_path_is_json_404`
  - `spa_fallback_serves_index_html` (for `GET /topics/3`)
  - `missing_question_is_404_with_error_shape`
  - `bind_error_mentions_port_flag`: occupy a port with a `TcpListener`, call `serve`, and assert the error text.
- [ ] **Step 2: Run `cargo test -p learning-tool --test web_api`.** Expected: FAIL.
- [ ] **Step 3: Implement.** Move `mod mcp;` into `lib.rs`; `main.rs` uses `learning_tool::mcp`.
- [ ] **Step 4: Run `cargo test --workspace`.** Expected: PASS. Also do a manual smoke test: `cargo run -p learning-tool -- serve --no-open` and `curl -s localhost:4747/api/home` should return JSON.
- [ ] **Step 5: Commit** with `git commit -m "Add learning-tool serve with the REST API"`.

---

### Task 7: TypeScript types and the frontend scaffold

**Files:**
- Create: `.cargo/config.toml`, and under `web/`: `package.json`, `pnpm-lock.yaml`, `vite.config.ts`, `tsconfig*.json`, `biome.json`, `components.json`, `index.html`, `src/main.tsx`, `src/styles.css`, `src/routes/__root.tsx`, `src/routes/index.tsx`, `src/routes/topics/index.tsx`, `src/routes/topics/$topicId.tsx`, `src/routes/questions/$questionId.tsx`, `src/api/client.ts`, `src/api/queries.ts`, `src/components/ErrorState.tsx`, `src/components/ui/*`, `src/test/setup.ts`
- Modify: `crates/core/Cargo.toml` and `crates/cli/Cargo.toml` (feature `ts`), `crates/core/src/types.rs` (`str_enum!`), plus `#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]` on every type that crosses the API

**Interfaces:**
- Produces:
  - **Generated types:**
    - `cargo test --workspace --features ts export_bindings` writes `web/src/api/generated/*.ts`. Add `--features ts` to the workspace test command in CLAUDE.md.
    - `[u32; 4]` becomes `[number, number, number, number]`, `NaiveDate` becomes `string`, and `DateTime<Utc>` becomes `string`.
  - **`client.ts`:**
    - `export class ApiError extends Error { kind: string; status: number }`
    - `export async function api<T>(method: "GET"|"POST"|"PATCH"|"PUT"|"DELETE", path: string, body?: unknown): Promise<T>`, which uses relative paths only (`/api/...`)
  - **`queries.ts`:**
    - Query keys: `["home"]`, `["activity", from, to]`, `["topic", id ?? "root", all, archived]`, `["question", id]`.
    - `homeQuery()`, `activityQuery(from, to)`, `topicQuery(id, all, archived)`, `questionQuery(id)`
    - Mutation hooks: `useCreateTopic`, `useUpdateTopic`, `useUpdateQuestion`, `useSetQuestionStatus`, `useRateQuestion`, `usePin`, `useUnpin`. Each invalidates `["home"]`, plus the affected `["topic", …]` and `["question", …]`.
  - **`ErrorState`:** props `{ error: unknown; onRetry: () => void }`. It shows the message and a Retry button.
  - **`__root.tsx`:**
    - navbar with Home, Topics, and Map/Projects disabled with a "coming soon" title
    - `<Outlet/>`
    - not-found component
    - `QueryClientProvider` lives in `main.tsx`
  - **`vite.config.ts`:** plugins `tanstackRouter()`, `react()`, `tailwindcss()`; `server.proxy['/api'] = 'http://127.0.0.1:4747'`; vitest `environment: 'jsdom'`, `setupFiles: ['src/test/setup.ts']`.
  - **shadcn components:** button, card, dialog, alert-dialog, tooltip, input, textarea, label, table, badge, switch, select, alert, skeleton.
  - **Scripts:** `pnpm dev`, `pnpm build` (`tsc -b && vite build`), `pnpm test` (`vitest run`), `pnpm check` (`biome check .`).

- [ ] **Step 1: Write failing tests:**
  - `src/api/client.test.ts`: `api_throws_ApiError_with_kind_and_status` (mock `fetch` returning 404 with the error JSON) and `api_returns_json_on_ok`.
  - `src/components/ErrorState.test.tsx`: `query_error_shows_retry`. The message is rendered, and clicking Retry calls `onRetry`.
  - `src/routes/__root.test.tsx`: `unknown_route_shows_not_found`.
- [ ] **Step 2: Run `pnpm --dir web test`.** Expected: FAIL.
- [ ] **Step 3: Implement** the scaffold, generate the types, and write the client, queries, `ErrorState` and the root layout. The three page routes render placeholder headings for now.
- [ ] **Step 4: Run `pnpm --dir web test && pnpm --dir web check && pnpm --dir web build`.** Expected: all succeed. Then run `cargo test --workspace --features ts` and confirm `git status` shows no changes under `web/src/api/generated/`.
- [ ] **Step 5: Commit** with `git commit -m "Scaffold the web app with generated API types"`.

---

### Task 8: Home page

**Files:**
- Create: `web/src/lib/heatmap.ts`, `web/src/components/home/{TodayTiles,Heatmap,ComingUp,NeedsAttention}.tsx` and their tests
- Modify: `web/src/routes/index.tsx`

**Interfaces:**
- Consumes: `homeQuery`, `activityQuery`, and the generated `HomeSummary` and `DayActivity`.
- Produces:
  - `export type HeatCell = { date: string; count: number; byGrade: [number,number,number,number]; topics: string[]; level: 0|1|2|3|4; isToday: boolean }`
  - `export function buildHeatmap(activity: DayActivity[], today: string): HeatCell[][]`:
    - 53 week-columns of 7 days each (Sunday–Saturday), ending with the week that contains `today`
    - `level` is 0 when the count is 0, then 1 (1–2), 2 (3–5), 3 (6–10), 4 (11 or more)
  - **`Heatmap`:**
    - an SVG with 10px cells and 3px gaps
    - each cell is a focusable `<rect>` with an `aria-label` like `"24 Oct 2026: 5 questions"`
    - a shadcn Tooltip shows the date, `"N questions"`, `"Easy a · Good b · Hard c · Again d"`, and `"Topics: …"`
    - today is outlined
  - **`TodayTiles`:** answered today (with the grade breakdown), remaining today (with "due tomorrow"), and streak with best streak.
  - **`ComingUp`:** tomorrow's load, new questions waiting (≈ days), and the pinned list linking to `/questions/$id`.
  - **`NeedsAttention`:** leeches, linked, and the count of unprocessed ratings with the hint "run /refine-questions in Claude Code".
  - The page shows a skeleton while loading and `ErrorState` on error.

- [ ] **Step 1: Write failing tests:**
  - `heatmap.test.ts`:
    - `buildHeatmap_has_53_weeks_of_7`
    - `levels_follow_thresholds`
    - `today_is_last_week_and_flagged`
    - `days_without_activity_are_zero`
  - `Heatmap.test.tsx`:
    - `heatmap_renders_empty_year` (371 or fewer focusable cells, no errors)
    - `tooltip_shows_grades_and_topics_on_focus`
  - `TodayTiles.test.tsx`: `shows_counts_breakdown_and_best_streak`
  - `ComingUp.test.tsx`: `pinned_items_link_to_question_pages`
- [ ] **Step 2: Run `pnpm --dir web test`.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `pnpm --dir web test && pnpm --dir web check`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add the home page with today tiles and activity heatmap"`.

---

### Task 9: Topic pages

**Files:**
- Create: `web/src/components/topics/{Breadcrumbs,TopicHeader,TopicDialogs,SubtopicCards,QuestionTable}.tsx` and their tests
- Modify: `web/src/routes/topics/index.tsx`, `web/src/routes/topics/$topicId.tsx`

**Interfaces:**
- Consumes: `topicQuery`, `useCreateTopic`, `useUpdateTopic`, `usePin`/`useUnpin`, and the generated `TopicPage`, `TopicCard` and `QuestionRow`.
- Produces:
  - **`Breadcrumbs`:** props `{ path: TopicRef[] }`. "Topics" always comes first and links to `/topics`; the last item isn't a link.
  - **`TopicHeader`:** name, target level, the summary line, and Rename / Archive-or-Unarchive / "+ Subtopic" buttons. On the root page only "+ Topic" appears.
  - **`TopicDialogs`:**
    - **Rename:** name (required) and description.
    - **Archive:** an AlertDialog. Its text says the questions stop being scheduled and the history is kept.
    - **New subtopic:** name, target level (0–3 select, default the parent's target), description.
    - Errors from the server appear inline.
  - **`SubtopicCards`:** a grid of links to `/topics/$id`, showing counts by level, studied/active, recall %, and a progress bar of studied/active.
  - **`QuestionTable`:** columns level, prompt (a link to the question page), status badge, next due, and a pin toggle button with `aria-pressed`. Prompts are truncated with CSS, with the full text in `title`.
  - Route search params: `all` and `archived` (booleans) via TanStack Router `validateSearch`. Toggles update the URL.

- [ ] **Step 1: Write failing tests:**
  - `Breadcrumbs.test.tsx`: `renders_links_except_last`
  - `SubtopicCards.test.tsx`: `card_links_and_progress`
  - `QuestionTable.test.tsx`:
    - `pin_toggle_calls_pin_then_unpin`
    - `long_prompt_is_truncated_with_title`
  - `TopicDialogs.test.tsx`:
    - `rename_requires_name`
    - `archive_dialog_explains_history_is_kept`
- [ ] **Step 2: Run `pnpm --dir web test`.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `pnpm --dir web test && pnpm --dir web check`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add topic pages with breadcrumbs, subtopics and editing"`.

---

### Task 10: Question page

**Files:**
- Create: `web/src/lib/rubric.ts`, `web/src/components/questions/{QuestionHeader,AnswerRubric,ScheduleBox,RatingBox,AttemptHistory,EditQuestionDialog,RubricEditor}.tsx` and their tests
- Modify: `web/src/routes/questions/$questionId.tsx`

**Interfaces:**
- Consumes: `questionQuery`, `useUpdateQuestion`, `useSetQuestionStatus`, `useRateQuestion`, `usePin`/`useUnpin`, and the generated `QuestionPage`, `ReviewRow`, `RubricPoint` and `QuestionPatch`.
- Produces:
  - **`rubric.ts`:** `export function validateRubric(points: RubricPoint[], passThreshold: number): string[]`. It returns error messages that mirror the core rules: at least one point; ids unique and non-empty; point text non-empty; weight > 0; threshold in (0, 1].
  - **`QuestionHeader`:**
    - the breadcrumb path, a level badge, and the prompt
    - buttons: Pin/Pinned (`aria-pressed`), Edit, Suspend/Unsuspend
    - a retired banner when `detail.question.status === "retired"`, with "This wording was replaced: see the newest version" linking to `newest_revision_id`
  - **`AnswerRubric`:** the answer, the rubric list (essential points marked ★, weights shown), the pass threshold %, and the source citation.
  - **`ScheduleBox`:** due date, reviews, lapses, and recall now %. A new question shows "Not studied yet".
  - **`RatingBox`:** 5 star buttons, an optional feedback textarea, and Submit (disabled until a star is chosen). Past ratings are listed below.
  - **`AttemptHistory`:**
    - one row per review, newest first: date, asked wording, answer (truncated), points ✓/✗ by id, score %, grade
    - each row expands (`<details>`) to show per-point notes and the feedback
    - rows from earlier revisions are labeled "earlier wording"
  - **`EditQuestionDialog`:**
    - fields: prompt, answer, level, pass threshold, and the `RubricEditor`
    - `RubricEditor` can add, remove and reorder points, and edit text, weight and the essential toggle; ids auto-assign `r<n>` for new points
    - shows the `validateRubric` errors inline
    - saving says "Rewording creates a new version; history is kept." On success it navigates to the returned question's id.

- [ ] **Step 1: Write failing tests:**
  - `rubric.test.ts`:
    - `valid_rubric_has_no_errors`
    - `rejects_empty_duplicate_zero_weight_and_bad_threshold`
  - `AttemptHistory.test.tsx`:
    - `rows_show_points_score_grade_newest_first`
    - `earlier_wording_labeled`
  - `RatingBox.test.tsx`: `submit_disabled_until_star_then_calls_mutation`
  - `QuestionHeader.test.tsx`: `retired_banner_links_newest`
  - `EditQuestionDialog.test.tsx`:
    - `editor_blocks_save_on_invalid_rubric`
    - `save_navigates_to_new_revision`
- [ ] **Step 2: Run `pnpm --dir web test`.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `pnpm --dir web test && pnpm --dir web check && pnpm --dir web build`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add the question page with history, rating, pinning and editing"`.

---

### Task 11: Build, install, docs and acceptance

**Files:**
- Modify: `scripts/install.sh`, `README.md`, `CLAUDE.md`, `docs/review-skills.md` (if needed), `.claude/skills/review-frontend/conventions.md`

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1:** Update `install.sh`:
  - require `pnpm` (and say how to install it)
  - run `pnpm --dir web install --frozen-lockfile && pnpm --dir web build` before `cargo install`
  - print `learning-tool serve` in the next steps
- [ ] **Step 2:** Update the docs:
  - **README:** add a **Web app** section: `learning-tool serve`, the port setting, what each page shows, and that quizzing stays in Claude Code.
  - **CLAUDE.md:** status ("Milestone 3 implemented"), layout (`crates/cli/src/web`, `web/`) and commands:
    - `pnpm --dir web dev` with `learning-tool serve --no-open`
    - `cargo test --workspace --features ts` to regenerate types
    - `pnpm --dir web test`
  - **FE-R conventions:** replace the "to expand once code exists" notes with what was actually built (query keys, `ErrorState`, route search params).
- [ ] **Step 3: Run `./scripts/install.sh`, then `learning-tool serve`.** Do the spec's manual acceptance steps 1–4 in the browser pane:
  1. The home page numbers match `learning-tool stats`.
  2. Breadcrumbs and subtopics navigate correctly.
  3. A pinned question comes first in `/study` and the new-question count drops by one.
  4. Rewording moves to the new revision and keeps the history.
- [ ] **Step 4:** Run `/review-backend`, `/review-database`, `/review-frontend` and `/review-code` on `main...HEAD`. Fix the high findings and the medium ones in new code, each with a test, and list what was left.
- [ ] **Step 5: Commit** with `git commit -m "Install the web app with the binary and document it"`.

---

## Spec coverage check

| Spec section | Task(s) |
|---|---|
| Serving (command, port, open browser, SPA fallback, embedding, placeholder, middleware, errors) | 6, 11 |
| Shared types (ts-rs, committed) | 7 |
| Pins (table, rules, used-on-record, unpin soft, MCP tools, skill) | 1, 2, 3 |
| Activity, best streak, overview, `studied` | 4 |
| Page view structs (no answers in lists, `newest_revision_id`) | 5 |
| HTTP API table | 6 |
| Frontend stack, routes, home, topic, question pages, edit dialog, states, themes | 7, 8, 9, 10 |
| Testing and manual acceptance; reviews | every task, 11 |
