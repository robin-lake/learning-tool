# Milestone 1: Study Loop + Topic Building Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust MCP server, an admin CLI, and three Claude Code skills (`/study`, `/build-topic`, `/refine-questions`). Robin can then build topics from references and study them daily with rubric grading and FSRS scheduling.

**Architecture:** A Cargo workspace with two crates:
- `learning-core`: a library holding all logic: SQLite via sqlx, FSRS scheduling, session building, references with FTS5, the question-writing guide, backups.
- `learning-tool`: the binary. A clap CLI whose `mcp` subcommand serves `learning-core` over stdio with `rmcp`.

The skills are Markdown files in `skills/`, symlinked into `~/.claude/skills/`. Every core function takes `now: DateTime<Utc>` as an argument (no hidden clock), so it can be tested deterministically.

**Tech Stack:** Rust 2024 edition (rustc 1.94). Crates: `tokio`, `sqlx` 0.9 (sqlite, macros, migrate, chrono), `fsrs` 6.6, `rmcp` 3.7 (server, transport-io; dev: client, transport-child-process), `schemars` 1, `chrono` 0.4, `chrono-tz` 0.10, `clap` 4, `serde`/`serde_json`, `thiserror` 2, `anyhow`, `tracing`/`tracing-subscriber`, `tempfile` (dev). External tools: `sqlx-cli`, `poppler` (`pdftotext`), `ocrmypdf`.

**Spec:** `docs/superpowers/specs/2026-10-23-milestone-1-study-loop-design.md`. Read it alongside this plan. Its "Data model", "Scheduling and session rules", "MCP tools", and "Skills" sections are normative.

## Global Constraints

- No `Co-Authored-By: Claude` or any Claude attribution in commit messages.
- The data directory is `~/.learning-tool/`, overridable with `LEARNING_TOOL_HOME`. Layout: `learning.db`, `references/<id>/`, `backups/`, `logs/`.
- stdout belongs to MCP. All logging goes to stderr or `logs/`, never stdout, in every subcommand.
- Timestamps are stored as UTC RFC 3339 text. The study day is local time with rollover at `day_rollover_hour` (default 4).
- Nothing is hard-deleted. Topics move to `archived`; questions move to `suspended` or `retired`. `reviews` is append-only (no UPDATE or DELETE statements on it anywhere).
- Settings defaults: `new_per_day=3`, `new_pause_review_threshold=20`, `backlog_daily_limit=30`, `session_target=10`, `desired_retention_main=0.9`, `leech_lapses=6`, `day_rollover_hour=4`, `timezone=system`, `backup_dir` (empty = `<data>/backups`), `backups_keep=2`.
- Levels are integers 0–3. Grades are 1 again, 2 hard, 3 good, 4 easy.
- The rubric and reference answer are returned to Claude by `next_question`. The skill must never show them to Robin before they answer.
- The core library package is named `learning-core` (the crate can't be named `core`; that clashes with Rust's built-in crate), and it lives in `crates/core/`.
- Don't add dependencies beyond the Tech Stack list without a reason recorded in the commit message.

## Review Focus

1. **Two Claude Code sessions run the MCP server at once.** Both write to the same database. Expect no "database is locked" errors: use WAL mode and `busy_timeout=5s`. *Test: Task 1, `concurrent_writers_do_not_fail`.*
2. **Claude retries `record_review` for an item already graded.** Expect a clear error and no duplicate review row. *Test: Task 7, `record_review_twice_is_rejected`.*
3. **A session started at 3:50 and continued past the 4:00 rollover.** Reviews count toward the session's study day, not the next one. *Test: Task 7, `session_keeps_its_study_day_across_rollover`.*
4. **Archiving a topic that has active questions.** Its questions stop being scheduled, and their history is kept. *Test: Task 6, `archived_topic_questions_are_not_queued`.*
5. **First run with an empty database.** `start_session` returns zero counts and a session that ends immediately, with no error. *Test: Task 7, `empty_database_session_is_empty`.*

---

## File Structure

```
Cargo.toml                         # workspace: members crates/core, crates/cli; shared dependency versions
.env                               # DATABASE_URL=sqlite:crates/core/dev.db (sqlx macros at build time)
.sqlx/                             # sqlx offline query data, committed
scripts/install.sh                 # cargo install + claude mcp add + skill symlinks
crates/core/
  Cargo.toml
  migrations/0001_schema.sql       # all tables, FTS5, triggers
  migrations/0002_settings.sql     # settings defaults
  seed/guide-v1.md                 # first question-writing guide (inserted at open if no guide exists)
  src/lib.rs                       # module exports, Error, Result
  src/error.rs                     # Error enum
  src/db.rs                        # DataDir, open(), WAL/busy_timeout, migrate, seed
  src/settings.rs                  # Settings struct load/get/set, TzSetting
  src/clock.rs                     # study_day()
  src/grading.rs                   # Rubric types, validate, score, grade mapping (pure)
  src/scheduler.rs                 # FSRS wrapper, retrievability, apply_review, rebuild_state
  src/topics.rs                    # topics + topic_links
  src/questions.rs                 # questions, revisions, status, links, load forecast
  src/queue.rs                     # pure queue building (selection, ordering, interleave)
  src/session.rs                   # start/next/record/end, session_items, leech
  src/stats.rs                     # today_stats, streak
  src/chunking.rs                  # pure text chunking
  src/references.rs                # add/search/read references
  src/guide.rs                     # guide versions, examples, ratings
  src/backup.rs                    # VACUUM INTO backups, pruning, restore
crates/cli/
  Cargo.toml
  src/main.rs                      # clap: mcp | migrate | stats | backup | restore | rebuild-state
  src/mcp/mod.rs                   # LearningServer, ServerHandler, serve over stdio
  src/mcp/params.rs                # tool parameter structs (serde + JsonSchema)
  src/mcp/tools.rs                 # #[tool_router] impl: thin wrappers over learning-core
  tests/mcp_stdio.rs               # end-to-end over stdio with the rmcp client
skills/study/SKILL.md
skills/build-topic/SKILL.md
skills/refine-questions/SKILL.md
```

Core unit tests live in each module's `#[cfg(test)] mod tests` and use a shared helper `db::test_pool() -> SqlitePool`: in-memory, migrated, seeded, max 1 connection.

---

### Task 1: Workspace, schema, database opening, settings

**Files:**
- Create: `Cargo.toml`, `.env`, `crates/core/Cargo.toml`, `crates/core/migrations/0001_schema.sql`, `crates/core/migrations/0002_settings.sql`, `crates/core/seed/guide-v1.md` (placeholder line `# Question-writing guide v1`; real content comes in Task 10), `crates/core/src/{lib,error,db,settings}.rs`, `crates/cli/Cargo.toml`, `crates/cli/src/main.rs` (just `fn main() {}` for now)
- Modify: `.gitignore` (add `/target`, `crates/core/dev.db*`, `.env` stays committed)

**Interfaces:**
- Produces:
  - `pub enum Error { Db(sqlx::Error), Migrate(sqlx::migrate::MigrateError), Io(std::io::Error), NotFound { entity: &'static str, id: i64 }, Invalid(String), Conflict(String), Fsrs(String) }`, plus `pub type Result<T> = std::result::Result<T, Error>`.
  - `pub struct DataDir { pub root: PathBuf }` with `DataDir::from_env() -> DataDir`, `db_path()`, `references_dir()`, `backups_dir()`, `logs_dir()`, `ensure() -> Result<()>`.
  - `pub async fn db::open(path: &Path) -> Result<SqlitePool>`: creates the file if missing; WAL; `busy_timeout` 5s; `foreign_keys` on; runs migrations; seeds guide v1 if `guide_versions` is empty.
  - `pub async fn db::test_pool() -> SqlitePool` (`#[cfg(any(test, feature = "test-util"))]`).
  - `pub struct Settings { new_per_day: u32, new_pause_review_threshold: u32, backlog_daily_limit: u32, session_target: u32, desired_retention_main: f32, leech_lapses: u32, day_rollover_hour: u32, timezone: TzSetting, backup_dir: Option<PathBuf>, backups_keep: u32 }` (all fields `pub`).
  - `pub enum TzSetting { System, Named(chrono_tz::Tz) }`.
  - `Settings::load(pool) -> Result<Settings>` and `settings::set(pool, key: &str, value: &str) -> Result<()>`. `set` validates the key and the value type, and returns `Error::Invalid` for unknown keys.

- [ ] **Step 1: Install sqlx-cli and write the workspace manifests.** Run `cargo install sqlx-cli --no-default-features --features sqlite`. The workspace `Cargo.toml` declares `[workspace.dependencies]` with the versions from the Tech Stack line, and `edition = "2024"`. sqlx features: `runtime-tokio`, `sqlite`, `macros`, `migrate`, `chrono`.

- [ ] **Step 2: Write `0001_schema.sql`.** Include every table in the spec's Data model with the columns and CHECK constraints listed there:
  - `topics`, `topic_links`, `questions`, `question_topics`, `question_links`, `reviews`, `question_state`, `sessions`, `question_ratings`, `guide_versions`, `guide_examples`
  - `sources`: the references table, named `sources` to avoid the SQL keyword. Columns as in the spec's `references`.
  - `reference_topics` (`source_id`, `topic_id`), `reference_chunks` (`source_id`, `locator`, `ordinal`, `text`)
  - `projects`, `project_topics`, `settings`
  - **plus** `session_items(id, session_id, ordinal, question_id, kind CHECK(kind IN ('review','new','requeue')), served_at, review_id)`. This is a builder addition: it persists the queue so `next_question` survives a server restart.

  CHECKs: `level BETWEEN 0 AND 3`, `grade BETWEEN 1 AND 4`, `usefulness BETWEEN 1 AND 5`, `weight BETWEEN 0 AND 1` on the link tables, and status/scope/kind enums as text CHECKs. For FTS, add `CREATE VIRTUAL TABLE reference_chunks_fts USING fts5(text, content='reference_chunks', content_rowid='id', tokenize='porter unicode61')` with insert, delete, and update triggers. Indexes: `questions(topic_id)`, `questions(status, scope)`, `question_state(due_on)`, `reviews(question_id, reviewed_at)`, `reviews(session_id)`, `session_items(session_id, ordinal)`, and a unique index on `topics(COALESCE(parent_id, 0), slug)`.

- [ ] **Step 3: Write `0002_settings.sql`** inserting the defaults from Global Constraints.

- [ ] **Step 4: Write failing tests in `db.rs` and `settings.rs`:**

```rust
#[tokio::test] async fn migrations_apply_and_fts_exists() {
    let pool = test_pool().await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE name = 'reference_chunks_fts'")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(n, 1);
}
#[tokio::test] async fn guide_v1_seeded_once() { /* open twice on the same temp file; assert count(guide_versions) == 1 */ }
#[tokio::test] async fn settings_defaults_load() {
    let s = Settings::load(&test_pool().await).await.unwrap();
    assert_eq!((s.new_per_day, s.new_pause_review_threshold, s.backlog_daily_limit, s.session_target), (3, 20, 30, 10));
    assert_eq!(s.desired_retention_main, 0.9); assert_eq!(s.leech_lapses, 6);
    assert_eq!(s.day_rollover_hour, 4); assert_eq!(s.backups_keep, 2);
    assert!(matches!(s.timezone, TzSetting::System));
}
#[tokio::test] async fn settings_set_rejects_unknown_key_and_bad_value() {
    let p = test_pool().await;
    assert!(matches!(set(&p, "nope", "1").await, Err(Error::Invalid(_))));
    assert!(matches!(set(&p, "new_per_day", "abc").await, Err(Error::Invalid(_))));
    set(&p, "timezone", "America/Los_Angeles").await.unwrap();
}
#[tokio::test] async fn concurrent_writers_do_not_fail() {
    // tempfile DB; open two pools on the same path; 50 interleaved INSERTs into settings-like scratch rows
    // (e.g. set(&p, "session_target", ..)) from two tokio tasks; both complete Ok.
}
```

- [ ] **Step 5: Run `cargo test -p learning-core`.** Expected: FAIL (functions not defined).
- [ ] **Step 6: Implement** `error.rs`, `db.rs` (embed migrations with `sqlx::migrate!("./migrations")`, seed with `include_str!("../seed/guide-v1.md")`), and `settings.rs`. Create the dev DB for the macros with `sqlx database create && sqlx migrate run --source crates/core/migrations`.
- [ ] **Step 7: Run `cargo test -p learning-core`.** Expected: PASS.
- [ ] **Step 8: Run `cargo sqlx prepare --workspace`** and confirm `SQLX_OFFLINE=true cargo build` succeeds.
- [ ] **Step 9: Commit** with `git commit -m "Add workspace, schema, database opening and settings"`.

---

### Task 2: Study day and grading (pure)

**Files:**
- Create: `crates/core/src/clock.rs`, `crates/core/src/grading.rs`

**Interfaces:**
- Produces:
  - `pub fn clock::study_day<Tz: TimeZone>(now: DateTime<Utc>, tz: &Tz, rollover_hour: u32) -> NaiveDate`: the local date of `now - rollover_hour hours`.
  - `impl Settings { pub fn study_day(&self, now: DateTime<Utc>) -> NaiveDate }` dispatches on `TzSetting` (`System` → `chrono::Local`).
  - `#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)] pub struct RubricPoint { pub id: String, pub point: String, #[serde(default = "one")] pub weight: f32, #[serde(default)] pub essential: bool }`
  - `pub struct Rubric(pub Vec<RubricPoint>)`
  - `pub struct PointResult { pub id: String, pub hit: bool, #[serde(default)] pub note: String }` (same derives)
  - `pub fn grading::validate_rubric(r: &Rubric, pass_threshold: f32) -> Result<()>`: non-empty; unique non-empty ids; non-empty point text; `weight > 0`; `0 < pass_threshold <= 1`.
  - `pub struct Score { pub score: f32, pub essentials_met: bool }`
  - `pub fn grading::score(r: &Rubric, results: &[PointResult]) -> Result<Score>`: `results` must cover exactly the rubric's ids, or `Error::Invalid`. `score` is exactly `1.0` when every point is hit.
  - `#[repr(u8)] pub enum Grade { Again = 1, Hard = 2, Good = 3, Easy = 4 }`
  - `pub fn grading::grade(s: &Score, pass_threshold: f32, hints_used: u32, fluent: bool) -> Grade` implements the spec's table, applied top to bottom.

- [ ] **Step 1: Write failing tests:**

```rust
// clock
let la = chrono_tz::America::Los_Angeles;
// 10:59Z = 03:59 PDT → before 04:00 → study day 2026-10-22
assert_eq!(study_day(utc("2026-10-23T10:59:00Z"), &la, 4), date(2026,10,22));
assert_eq!(study_day(utc("2026-10-23T11:00:00Z"), &la, 4), date(2026,10,23));
// DST end 2026-11-01 (01:00–02:00 repeats): 2026-11-01T11:30Z = 03:30 PST → 2026-10-31
assert_eq!(study_day(utc("2026-11-01T11:30:00Z"), &la, 4), date(2026,10,31));
assert_eq!(study_day(utc("2026-11-01T12:00:00Z"), &la, 4), date(2026,11,1));
// DST start 2026-03-08
assert_eq!(study_day(utc("2026-03-08T10:30:00Z"), &la, 4), date(2026,3,7)); // 03:30 PDT

// grading — rubric r1(w1, essential), r2(w1), r3(w2); threshold 0.6
grade_missing_essential_is_again          // r1 miss, r2+r3 hit (score .75) → Again
grade_below_threshold_is_again            // r1 hit only (score .25) → Again
grade_hint_is_hard                        // all hit, hints 1 → Hard
grade_low_pass_is_hard                    // r1+r3 hit, score .75 < .6+.2=.8 → Hard
grade_solid_pass_is_good                  // r1,r2,r3 hit, fluent=false → Good (score 1.0, not fluent)
grade_perfect_fluent_is_easy              // all hit, hints 0, fluent → Easy
grade_boundary_at_midpoint_is_good        // construct score == 0.8 exactly → Good
score_rejects_missing_or_extra_ids        // Error::Invalid
validate_rubric_rejects_dupes_empty_zero_weight_bad_threshold
```

- [ ] **Step 2: Run `cargo test -p learning-core clock grading`.** Expected: FAIL.
- [ ] **Step 3: Implement both modules.**
- [ ] **Step 4: Run the tests again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add study-day calculation and rubric grading"`.

---

### Task 3: Topics and topic links

**Files:**
- Create: `crates/core/src/topics.rs`

**Interfaces:**
- Produces:
  - `pub struct Topic { id, parent_id: Option<i64>, name, slug, description: String, scope: Scope, project_id: Option<i64>, target_level: u8, status: TopicStatus, position: i64 }`
  - `pub enum Scope { Main, Project }` and `pub enum TopicStatus { Active, Archived }`. These and all other enums serialize as lowercase strings matching the DB.
  - `pub struct NewTopic { parent_id: Option<i64>, name: String, description: String, target_level: u8, position: Option<i64> }`. The slug comes from the name (lowercase, alphanumerics, `-`). Scope is inherited from the parent; it's `Main` at top level.
  - `pub async fn create(pool, t: NewTopic, now) -> Result<Topic>` returns `Error::Conflict` on a duplicate sibling slug.
  - `pub async fn update(pool, id, patch: TopicPatch, now) -> Result<Topic>`, where `TopicPatch` has optional `name`, `description`, `target_level`, `status`, `position`, `parent_id`. Reparenting under one of the topic's own descendants returns `Error::Invalid`.
  - `pub async fn get(pool, id) -> Result<TopicDetail>`. `TopicDetail { topic, path: Vec<Topic> /* root..self */, children: Vec<Topic>, links: Vec<TopicLink>, source_ids: Vec<i64>, question_ids: Vec<i64> }`.
  - `pub async fn tree(pool, root: Option<i64>, depth: Option<u32>, now) -> Result<Vec<TopicNode>>`. `TopicNode { topic, depth: u32, questions_by_level: [u32; 4], active_questions: u32, avg_retrievability: Option<f32> /* over reviewed active questions in subtree */, children: Vec<TopicNode> }`. Uses a recursive CTE; `avg_retrievability` uses `scheduler::retrievability` (Task 4). Until Task 4 lands, return `None`; Task 4's tests cover it.
  - `pub async fn link(pool, from, to, kind: LinkKind, weight: f32, note: String) -> Result<TopicLink>`, with `LinkKind { Related, Prerequisite, Applies }`. `Related` normalizes so that `from < to`. Self-links and weights outside `[0,1]` return `Error::Invalid`. A repeat of the same `(from, to, kind)` updates the weight.

- [ ] **Step 1: Write failing tests:** `create_nested_and_get_path` (path is root→child→grandchild); `duplicate_sibling_slug_conflicts`; `same_slug_under_different_parents_ok`; `reparent_into_descendant_rejected`; `related_link_normalized_and_upserted` (`link(5,2)` then `link(2,5)` → one row, from=2, latest weight); `self_link_and_bad_weight_rejected`; `tree_counts_by_level_and_depth_limit`.
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add topics and weighted topic links"`.

---

### Task 4: Scheduler (FSRS wrapper, state rebuild)

**Files:**
- Create: `crates/core/src/scheduler.rs`
- Modify: `crates/core/src/topics.rs`, so `tree` fills `avg_retrievability`.

**Interfaces:**
- Consumes: `Grade` (Task 2).
- Produces:
  - `pub struct MemState { pub stability: f32, pub difficulty: f32 }`
  - `pub struct QState { question_id: i64, mem: Option<MemState>, due_on: Option<NaiveDate>, last_reviewed_at: Option<DateTime<Utc>>, last_study_day: Option<NaiveDate>, reps: u32, lapses: u32, state: CardState }`, with `CardState { New, Review, Relearning }`.
  - `pub struct SchedParams { pub desired_retention: f32, pub max_interval_days: Option<u32> }`
  - `pub fn retrievability(stability: f32, days_elapsed: f32) -> f32`. This is the FSRS-6 forgetting curve `R = (1 + factor·t/S)^(-decay)` with `factor = 0.9^(-1/decay) - 1`, where `decay` is the default parameter `w[20]` (take it from the crate's default parameter array; if the crate doesn't expose one, copy the 21 default values from fsrs 6.6 into a `const` with a comment).
  - `pub fn next(prev: &QState, grade: Grade, study_day: NaiveDate, reviewed_at: DateTime<Utc>, p: &SchedParams) -> Result<QState>`:
    - `days_elapsed` = study-day difference since `last_study_day` (0 for new or same-day).
    - Call `FSRS::default().next_states(prev.mem, p.desired_retention, days_elapsed)` and pick the state for the grade.
    - `interval = max(1, round(item.interval))`, capped by `max_interval_days`. `due_on = study_day + interval`.
    - `reps += 1`.
    - A lapse is `Again` when `prev.state == Review`: increment `lapses` and set state to `Relearning`. `Again` on `New` stays `Relearning` with no lapse. Any pass → `Review`.
  - `pub async fn load_state(pool, question_id) -> Result<QState>` returns a `New` default if there's no row.
  - `pub async fn save_state(conn: &mut SqliteConnection, s: &QState) -> Result<()>` upserts.
  - `pub async fn rebuild_all(pool, settings: &Settings) -> Result<u32>`: truncates `question_state`, replays `reviews` ordered by `(question_id, reviewed_at, id)` through `next` using each review's session `study_day`, and returns the number of questions rebuilt. Milestone 1 uses `SchedParams { desired_retention: settings.desired_retention_main, max_interval_days: None }` for all questions. Project parameters are a later milestone.

- [ ] **Step 1: Write failing tests:**
  - `retrievability_is_point_nine_at_stability` (`|R(S=10,t=10) - 0.9| < 1e-4`)
  - `retrievability_decreases_with_time`
  - `first_good_schedules_future_due` (`due_on > study_day`, `reps == 1`, state Review)
  - `again_on_review_is_lapse`
  - `again_on_new_is_not_lapse`
  - `easy_interval_ge_good_interval`
  - `max_interval_caps`
  - `same_day_review_uses_zero_elapsed` (no error)
  - `rebuild_matches_incremental`: record 6 reviews across 3 questions through `next` + `save_state`, snapshot `question_state`, run `rebuild_all`, then assert identical rows
  - `tree_avg_retrievability_present_after_review`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add FSRS scheduler with replayable state"`.

---

### Task 5: Questions

**Files:**
- Create: `crates/core/src/questions.rs`

**Interfaces:**
- Consumes: `Rubric`, `validate_rubric` (Task 2); `Topic`/`Scope` (Task 3); `load_state` (Task 4).
- Produces:
  - `pub struct Question { id, topic_id, level: u8, prompt, answer, rubric: Rubric, pass_threshold: f32, source_id: Option<i64>, source_locator: Option<String>, scope: Scope, project_id: Option<i64>, status: QuestionStatus, priority: i64, guide_version_id: Option<i64>, revision_of: Option<i64>, created_at, updated_at }`, with `QuestionStatus { Proposed, Active, Suspended, Retired }`.
  - `pub struct NewQuestion { level: u8, prompt: String, answer: String, rubric: Rubric, pass_threshold: Option<f32> /* default 0.6 */, source_id: Option<i64>, source_locator: Option<String>, priority: Option<i64>, guide_version_id: Option<i64>, secondary_topic_ids: Vec<i64> }`
  - `pub struct AddResult { pub ids: Vec<i64>, pub forecast: LoadForecast }`
  - `pub struct LoadForecast { pub new_active: u32, pub days_of_new_capacity: u32 /* ceil(new_active / new_per_day) */, pub est_extra_reviews_per_day_first_month: f32 /* new_active * 8.0 / 30.0, per research (~7–10 reviews per new item in month one) */ }`
  - `pub async fn add(pool, topic_id, qs: Vec<NewQuestion>, status: QuestionStatus /* Active or Proposed only */, settings, now) -> Result<AddResult>`:
    - Runs in one transaction, all-or-nothing.
    - Validates each question. An empty prompt or answer, or a level outside 0–3, returns `Error::Invalid`, and the message names the question's index.
    - The default priority continues after the topic's current max, in input order.
    - Scope is inherited from the topic.
  - `pub async fn update(pool, id, patch: QuestionPatch, now) -> Result<Question>`. A change to `prompt`, `answer`, `rubric`, `pass_threshold`, or `level` creates a **revision**:
    - insert a new row with `revision_of = id`, keeping status, priority, topic, and source
    - mark the old row `retired`
    - copy the old row's `question_state` to the new id, so the schedule continues
    - return the new question

    Changes only to `priority`, `topic_id`, or `source_*` update in place.
  - `pub async fn set_status(pool, id, status, now) -> Result<()>`
  - `pub async fn link(pool, a, b, kind: LinkKind, weight, note) -> Result<()>`, with the same rules as topic links.
  - `pub async fn get(pool, id) -> Result<QuestionDetail>`. `QuestionDetail { question, topic_path: Vec<String>, state: QState, reviews: Vec<ReviewRow>, ratings: Vec<RatingRow>, revisions: Vec<i64> /* ancestor chain ids */ }`. `ReviewRow` and `RatingRow` mirror their tables; define them here so Tasks 7 and 10 reuse them.

- [ ] **Step 1: Write failing tests:**
  - `add_batch_assigns_priorities_and_forecast` (5 active, `new_per_day` 3 → `days_of_new_capacity` 2 and `est_extra_reviews_per_day_first_month` ≈ 1.333)
  - `add_proposed_not_counted_in_forecast` (`new_active == 0`)
  - `add_invalid_question_rolls_back_all` (second of three has an empty answer → `Err`, `count(questions) == 0`)
  - `add_inherits_topic_scope`
  - `rewording_creates_revision_and_carries_state`
  - `priority_change_updates_in_place`
  - `get_returns_history_and_revision_chain`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add questions with revisions and load forecast"`.

---

### Task 6: Queue building (pure + DB candidate loading)

**Files:**
- Create: `crates/core/src/queue.rs`

**Interfaces:**
- Consumes: `Settings`, `retrievability`, `QState`.
- Produces:
  - `pub struct Candidate { pub question_id: i64, pub topic_id: i64, pub topic_depth: u32, pub topic_position: i64, pub level: u8, pub priority: i64, pub retrievability: Option<f32> /* None for new */ }`
  - `pub struct QueuePlan { pub items: Vec<(i64, ItemKind)>, pub due_total: u32, pub backlog_carried: u32, pub new_allowed: u32 }`, with `ItemKind { Review, New, Requeue }`.
  - `pub fn plan(due: Vec<Candidate>, new: Vec<Candidate>, new_introduced_today: u32, s: &Settings) -> QueuePlan`. The algorithm, which the tests pin:
    1. Sort `due` by retrievability ascending. If `due.len() > backlog_daily_limit`, keep the first `backlog_daily_limit` and set `backlog_carried` to the rest.
    2. `new_allowed = if due_total >= new_pause_review_threshold { 0 } else { new_per_day.saturating_sub(new_introduced_today) }`. Sort `new` by `(topic_depth, topic_position, level, priority, question_id)` and take `new_allowed`.
    3. **Warm-up:** take up to 2 selected reviews with the highest retrievability, where R ≥ 0.85. They go first.
    4. **Body:** the remaining reviews, in ascending-retrievability order, with new items inserted evenly. Insert new item k (0-based, n total) at body index `round((k+1) * body_len / (n+1))`.
    5. **De-cluster** the warm-up plus body: for each i ≥ 1 where `topic(i) == topic(i-1)`, swap item i with the nearest later item whose topic differs from both `topic(i-1)` and `topic(i+1)`. If there isn't one, leave it.
  - `pub async fn load_candidates(pool, s: &Settings, today: NaiveDate, now) -> Result<(Vec<Candidate>, Vec<Candidate>, u32)>`. Returns `(due, new, new_introduced_today)`.
    - Only `status='active'`, `scope='main'` questions whose topic **and all of its ancestors** are `active`.
    - `due` means `due_on <= today`. `new` means no `question_state` row, or `state='new'`.
    - `new_introduced_today` = the number of main questions whose first review falls on `today`.

- [ ] **Step 1: Write failing tests (pure `plan`):**
  - `new_capped_by_per_day_minus_introduced` (3 per day, 1 introduced → 2)
  - `new_paused_when_due_at_threshold` (20 due → 0 new)
  - `backlog_limited_lowest_r_first` (35 due → 30 kept, the lowest R values, `backlog_carried` 5)
  - `new_ordered_broad_first` (depth 0 L1 before depth 1 L0; same depth: L0 before L1)
  - `warmup_takes_up_to_two_high_r`
  - `new_spread_evenly` (6 reviews + 2 new → new at body indices 2 and 4 after warm-up)
  - `adjacent_same_topic_declustered`
  - `all_same_topic_left_alone` (no panic, order stable)
- [ ] **Step 2: Write failing DB tests:**
  - `archived_topic_questions_are_not_queued` (archive a parent topic → child topic's active questions are absent from both lists, and `get` still returns their reviews)
  - `suspended_and_proposed_not_queued`
  - `new_introduced_today_counts_first_reviews_only`
- [ ] **Step 3: Run them.** Expected: FAIL.
- [ ] **Step 4: Implement.**
- [ ] **Step 5: Run them again.** Expected: PASS.
- [ ] **Step 6: Commit** with `git commit -m "Add session queue planning"`.

---

### Task 7: Session lifecycle

**Files:**
- Create: `crates/core/src/session.rs`

**Interfaces:**
- Consumes: `queue::{load_candidates, plan}`, `scheduler::{load_state, next, save_state}`, `grading::{score, grade}`, `questions::get`, `Settings::study_day`, `stats::streak` (Task 8; until then, return 0 and fill it in Task 8).
- Produces:
  - `pub struct SessionStart { pub session_id: i64, pub study_day: NaiveDate, pub planned: u32, pub due_total: u32, pub new_allowed: u32, pub backlog_carried: u32, pub streak: u32 }`
  - `pub async fn start(pool, s: &Settings, now) -> Result<SessionStart>`:
    - Creates a `sessions` row (kind `main`, with its `study_day`) and its `session_items`.
    - If an open (not ended) main session exists for the same study day, **resume it**: return it, and re-plan only if it has no unserved items.
  - `pub struct NextQuestion { pub item_id: i64, pub kind: ItemKind, pub position: u32, pub remaining: u32, pub question: Question, pub topic_path: Vec<String>, pub recent_reviews: Vec<ReviewRow> /* last 3 */, pub lapses: u32, pub is_leech: bool, pub source_citation: Option<String> /* "<title>, <locator>" */ }`
  - `pub async fn next_question(pool, session_id, now) -> Result<Option<NextQuestion>>`:
    - Returns the lowest-ordinal item with `review_id IS NULL` and sets `served_at`.
    - Serving again without a review returns the same item, which makes it idempotent.
    - Skips items whose question has since become non-active.
  - `pub struct ReviewInput { pub question_id: i64, pub rubric_result: Vec<PointResult>, pub hints_used: u32, pub fluent: bool, pub user_answer: String, pub asked_prompt: String, pub feedback: String, pub duration_ms: Option<i64> }`
  - `pub struct ReviewOutcome { pub review_id: i64, pub grade: Grade, pub score: f32, pub due_on: NaiveDate, pub lapses: u32, pub is_leech: bool, pub requeued: bool }`
  - `pub async fn record_review(pool, s: &Settings, session_id, input: ReviewInput, now) -> Result<ReviewOutcome>`:
    - In one transaction: find the session's unreviewed item for `question_id`. If there's none, return `Error::Conflict("already recorded or not in session")`.
    - Score and grade, insert into `reviews`, then compute `scheduler::next` using the **session's** `study_day`, and save the state.
    - Set `item.review_id`.
    - If the grade is `Again` and the item kind isn't `Requeue`, append a `Requeue` item at the end.
    - `is_leech = lapses >= leech_lapses`.
  - `pub struct SessionSummary { pub answered: u32, pub by_grade: [u32; 4], pub streak: u32, pub due_tomorrow: u32, pub leeches: Vec<i64> }`
  - `pub async fn end(pool, s, session_id, now) -> Result<SessionSummary>`. Sets `ended_at`. Ending twice returns the same summary.

- [ ] **Step 1: Write failing tests:**
  - `empty_database_session_is_empty` (`planned == 0`; `next_question` → `None`; `end` ok)
  - `full_session_flow` (2 due + 1 new → serve, record good on all → `answered == 3`, `due_on` in the future)
  - `again_requeues_once` (Again → item appended; Again on the requeue → no further item)
  - `record_review_twice_is_rejected` (second call → `Error::Conflict`; `count(reviews) == 1`)
  - `next_question_is_idempotent_until_recorded`
  - `start_resumes_open_session_same_day`
  - `session_keeps_its_study_day_across_rollover` (start at 03:50 local, record at 04:10 → review's `due_on` computed from the session's study day; stats count it on that day)
  - `leech_flag_at_threshold` (force `lapses` to 5, then a lapse → `is_leech`)
  - `unicode_and_multiline_answers_round_trip` (emoji, newlines, quotes in `user_answer` survive)
  - `rubric_mismatch_rejected_without_writing`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add study session lifecycle"`.

---

### Task 8: Stats and streak

**Files:**
- Create: `crates/core/src/stats.rs`
- Modify: `crates/core/src/session.rs` (use `streak`)

**Interfaces:**
- Produces:
  - `pub async fn streak(pool, s: &Settings, now) -> Result<u32>`. A study day qualifies if, among `main` sessions with that `study_day`, either:
    - reviews recorded ≥ `session_target`, or
    - some session has `ended_at` set and all of its items reviewed.

    The streak counts consecutive qualifying days ending at today. If today doesn't qualify yet, it counts back from yesterday.
  - `pub struct TodayStats { pub study_day: NaiveDate, pub answered_today: u32, pub remaining_today: u32, pub new_introduced_today: u32, pub due_tomorrow: u32, pub streak: u32 }`
  - `pub async fn today(pool, s, now) -> Result<TodayStats>`. `remaining_today` = unreviewed items in today's open session if there is one, otherwise the length of `plan(load_candidates(..))`.

- [ ] **Step 1: Write failing tests:**
  - `streak_counts_consecutive_days`
  - `streak_not_broken_before_today_done` (yesterday and the day before qualify, today has nothing → 2)
  - `short_day_with_all_due_done_qualifies` (3 due, session ended with all reviewed → qualifies)
  - `gap_resets_streak`
  - `today_stats_remaining_matches_plan`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add today stats and streak"`.

---

### Task 9: References (chunking, storage, search)

**Files:**
- Create: `crates/core/src/chunking.rs`, `crates/core/src/references.rs`

**Interfaces:**
- Produces:
  - `pub struct Chunk { pub locator: String, pub text: String }`
  - `pub fn chunking::chunk(text: &str, max_chars: usize /* 4000 */) -> Vec<Chunk>`:
    - If the text contains `\x0c` (form feed), split on it into pages with locator `p. N` (1-based). Skip empty pages, but keep the page numbers.
    - Otherwise split on Markdown headings (`#`… at line start). The locator is the heading path joined with ` > `, or `§1`… for text before the first heading.
    - Otherwise use one chunk, `§1`.
    - Any chunk longer than `max_chars` is split at the last blank line (falling back to the last newline, then to a hard cut) before the limit. Suffix the pieces ` (part k)`.
  - `pub struct Source { id, title, authors, kind: SourceKind, url: Option<String>, file_path: Option<String>, text_path: Option<String>, summary: String, status, added_at }`, with `SourceKind { Pdf, Article, Book, Docs, Video, Other }`.
  - `pub struct NewSource { title, authors: String, kind, url: Option<String>, source_file: Option<PathBuf>, text_file: Option<PathBuf>, summary: String, topic_ids: Vec<i64> }`
  - `pub async fn add(pool, dirs: &DataDir, n: NewSource, now) -> Result<(Source, u32 /* chunk count */)>`:
    - Copy `source_file` to `references/<id>/original.<ext>` and `text_file` to `references/<id>/text.md`. Paths are stored relative to the data root.
    - Chunk and index the text.
    - A missing input file returns `Error::Invalid` naming the path, and nothing is written. (Insert the row in a transaction; copy the files before committing; remove the directory if anything fails.)
  - `pub struct Hit { pub source_id: i64, pub title: String, pub locator: String, pub snippet: String, pub rank: f64 }`
  - `pub async fn search(pool, query: &str, topic_id: Option<i64>, limit: u32 /* default 10 */) -> Result<Vec<Hit>>`:
    - FTS5 `MATCH` ordered by `bm25`. `snippet` uses `snippet(reference_chunks_fts, 0, '[', ']', '…', 24)`.
    - `topic_id` filters through `reference_topics`, including descendant topics.
    - Escape the user's query as FTS5 string tokens (wrap each whitespace-separated term in double quotes, doubling any internal quotes) so input like `C++` or `"mouth` can't cause a syntax error.
  - `pub async fn get(pool, id) -> Result<(Source, Vec<i64> /* topic ids */, u32 /* chunks */)>`
  - `pub async fn read(pool, id, from_locator: Option<&str>, max_chunks: u32 /* default 5 */) -> Result<Vec<Chunk>>` returns chunks in ordinal order, starting at the matching locator.

- [ ] **Step 1: Write failing tests (chunking):**
  - `pdftotext_pages_get_page_locators` (`"a\x0cb\x0c\x0cd"` → `p. 1`, `p. 2`, `p. 4`)
  - `markdown_heading_paths` (`# A`, `## B` → `A > B`)
  - `plain_text_single_chunk`
  - `long_chunk_split_on_blank_line_with_part_suffix`
- [ ] **Step 2: Write failing tests (DB, with a temp `DataDir`):**
  - `add_copies_files_and_indexes`
  - `search_finds_term_with_locator_and_snippet`
  - `search_with_special_chars_does_not_error` (`C++`, `"mouth`, `horn*`)
  - `search_filters_by_topic_subtree`
  - `missing_file_writes_nothing`
  - `non_ascii_text_searchable` (`Schrödinger`)
  - `read_from_locator`
- [ ] **Step 3: Run them.** Expected: FAIL.
- [ ] **Step 4: Implement.**
- [ ] **Step 5: Run them again.** Expected: PASS.
- [ ] **Step 6: Commit** with `git commit -m "Add references with chunked full-text search"`.

---

### Task 10: Ratings and question-writing guide

**Files:**
- Create: `crates/core/src/guide.rs`
- Modify: `crates/core/seed/guide-v1.md` (real content)

**Interfaces:**
- Produces:
  - `pub enum RatingClass { QuestionQuality, KnowledgeValue, Positive }`
  - `pub async fn rate(pool, question_id, usefulness: u8, feedback: Option<String>, class: Option<RatingClass>, review_id: Option<i64>, now) -> Result<i64>`. `usefulness` outside 1–5 returns `Error::Invalid`.
  - `pub async fn classify(pool, rating_id, class: RatingClass) -> Result<()>`
  - `pub struct RatingView { rating: RatingRow, question_prompt: String, level: u8, topic_path: Vec<String>, guide_version_id: Option<i64> }`
  - `pub async fn list_ratings(pool, unprocessed_only: bool) -> Result<Vec<RatingView>>`
  - `pub struct GuideVersion { id, content: String, change_summary: String, parent_id: Option<i64>, created_at }` and `pub struct GuideExample { question_id, verdict: Verdict /* Good|Bad */, level: u8, reason: String, prompt: String }`
  - `pub async fn current(pool) -> Result<(GuideVersion, Vec<GuideExample>)>`
  - `pub async fn save_version(pool, content: String, change_summary: String, examples: Vec<NewExample>, processed_rating_ids: Vec<i64>, now) -> Result<GuideVersion>`:
    - Runs in one transaction.
    - `parent_id` is the current version.
    - Marks the given ratings as processed in the new version.
    - Examples referencing a missing question return `Error::Invalid`.
- **Seed content (`guide-v1.md`).** Write it from the spec's level definitions (CLAUDE.md "Mastery levels") and `docs/research/spaced-repetition.md`, section "Writing good items". Sections:
  - Purpose
  - Level definitions L0–L3, each with what a question at that level asks you to do and the typical rubric size (L0: 1–2 points; L1: 2–3; L2: 3–4 including a worked step; L3: 4–6, constraint-driven)
  - Item-writing rules
  - Rubric rules: points are independently checkable; mark as essential the points without which the answer is wrong; `pass_threshold` 0.6 for L0–L1, 0.7 for L2–L3
  - What's worth asking (main program = "matters in 10 years"; broad before specific)
  - Rephrasing rule (vary the surface, keep the target)

- [ ] **Step 1: Write failing tests:**
  - `rate_validates_range`
  - `list_unprocessed_excludes_processed`
  - `save_version_chains_parent_and_marks_processed`
  - `current_returns_latest_with_examples`
  - `seeded_guide_has_level_sections` (content contains `L0`, `L1`, `L2`, `L3`)
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement, and write the seed guide.**
- [ ] **Step 4: Run them again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add usefulness ratings and versioned question guide"`.

---

### Task 11: Backups and restore

**Files:**
- Create: `crates/core/src/backup.rs`

**Interfaces:**
- Produces:
  - `pub async fn backup(pool, dirs, s: &Settings, reason: &str /* "daily" | "pre-migrate" | "manual" */, now) -> Result<PathBuf>`:
    - Runs `VACUUM INTO '<backup_dir>/learning-<YYYYMMDD-HHMMSS>-<reason>.db'`.
    - Then prunes `learning-*.db` in that directory, keeping the newest `backups_keep` by name.
  - `pub async fn daily_if_needed(pool, dirs, s, now) -> Result<Option<PathBuf>>` takes a backup only if none in the directory has today's local date (`YYYYMMDD` of `now` in the configured timezone) in its name.
  - `pub fn restore(dirs: &DataDir, from: &Path) -> Result<()>`:
    - Refuses unless `from` opens as SQLite and contains a `reviews` table.
    - Moves the current DB (and any `-wal`/`-shm` files) to `learning.db.before-restore-<ts>`, then copies `from` into place.
    - The caller must ensure no server is running. The CLI prints this warning.
  - `db::open` gains a hook: if migrations are pending on an **existing** non-empty DB, it calls `backup(.., "pre-migrate", ..)` before migrating. Use `Migrator::iter()` against `_sqlx_migrations` to detect pending migrations.

- [ ] **Step 1: Write failing tests:**
  - `backup_creates_openable_copy`
  - `prune_keeps_two_newest`
  - `daily_if_needed_skips_second_call_same_day`
  - `restore_rejects_non_sqlite`
  - `restore_round_trip_preserves_reviews`
  - `pre_migrate_backup_only_when_pending`: simulate by opening a DB migrated with the first migration only. Use a `Migrator` built from a temp dir containing a copy of `0001_schema.sql`, then `db::open` → one `pre-migrate` file.
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run them again.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add daily and pre-migration backups with restore"`.

---

### Task 12: CLI and MCP server

**Files:**
- Create: `crates/cli/src/mcp/{mod,params,tools}.rs`, `crates/cli/tests/mcp_stdio.rs`
- Modify: `crates/cli/src/main.rs`, `crates/cli/Cargo.toml`

**Interfaces:**
- Consumes: every public `learning-core` function above.
- Produces:
  - Binary `learning-tool` with subcommands:
    - `mcp`: open the DB, run `daily_if_needed`, then serve.
    - `migrate`
    - `stats`: prints `TodayStats` as aligned text.
    - `backup`
    - `restore <file>` (requires `--yes`; warns to quit Claude Code first)
    - `rebuild-state`
    - `config get|set <key> [value]`
  - Logging: `tracing-subscriber` to stderr; under `mcp`, also to `logs/learning-tool.log`.
  - `struct LearningServer { pool: SqlitePool, dirs: DataDir, tool_router: ToolRouter<Self> }` implementing `ServerHandler`, following the rmcp 3 README pattern (`#[tool_router]`, `#[tool]`, `#[tool_handler]`, `Parameters<T>`, `serve(stdio())`). Each tool loads `Settings` fresh, so `config set` takes effect without a restart.
  - **Tools, one per spec "MCP tools" entry**, as snake_case names mapped to core:
    - `start_session` → `session::start`
    - `next_question` → `session::next_question`
    - `record_review` → `session::record_review`
    - `end_session` → `session::end`
    - `rate_question` → `guide::rate`
    - `list_topics` → `topics::tree`
    - `get_topic` → `topics::get`
    - `create_topic`, `update_topic`, `link_topics`
    - `add_questions` → `questions::add`
    - `update_question`, `set_question_status`, `link_questions`, `get_question`
    - `today_stats` → `stats::today`
    - `add_reference` → `references::add`
    - `search_references`, `get_reference`, `read_reference`
    - `get_guide` → `guide::current`
    - `list_ratings`, `classify_rating`, `save_guide_version`
  - Each tool's `description` states what it does and, for `next_question`, includes: "The rubric and answer are for grading only — never show them to the user before they answer."
  - **Results:** serialize the core return value as pretty JSON in a text content block, preceded by a one-line summary (for example `Session 12: 9 planned (7 due, 2 new). Streak 4.`).
  - **Errors:** core `Invalid`, `NotFound`, and `Conflict` errors become `CallToolResult::error` with the message, so Claude can see and fix them. Other errors become `ErrorData::internal_error`.

- [ ] **Step 1: Write the failing end-to-end test `crates/cli/tests/mcp_stdio.rs`.** It spawns `env!("CARGO_BIN_EXE_learning-tool") mcp` with `LEARNING_TOOL_HOME` set to a tempdir, using rmcp's `TokioChildProcess` client (dev-dependency features `client`, `transport-child-process`), then:
  - `list_tools` includes all 24 names above
  - `create_topic` → `add_questions` (2 active questions with rubrics) → `start_session` (planned 2) → `next_question` → `record_review` (all points hit, fluent) → grade 4 → `next_question` → `record_review` → `next_question` returns null → `end_session` answered 2
  - `add_reference` with a temp text file containing `"horn mouth\x0cthroat"` → `search_references("throat")` hit locator `p. 2`
  - `record_review` with an unknown rubric id → result `is_error == true`, and the message mentions the id
  - a backup file exists in `<tmp>/backups/` after startup
- [ ] **Step 2: Run `cargo test -p learning-tool --test mcp_stdio`.** Expected: FAIL.
- [ ] **Step 3: Implement the param structs** (serde + `schemars::JsonSchema`, with doc comments; the doc comments become the tool schemas Claude reads), the tools, the server, and the CLI.
- [ ] **Step 4: Run `cargo test --workspace`.** Expected: PASS.
- [ ] **Step 5: Smoke-test the CLI.** Run `LEARNING_TOOL_HOME=$(mktemp -d) cargo run -p learning-tool -- stats`. Expected: prints the study day, `answered 0`, `streak 0`, and nothing else on stdout.
- [ ] **Step 6: Commit** with `git commit -m "Add CLI and MCP server"`.

---

### Task 13: Skills, install, docs, dogfood

**Files:**
- Create: `skills/study/SKILL.md`, `skills/build-topic/SKILL.md`, `skills/refine-questions/SKILL.md`, `scripts/install.sh`
- Modify: `README.md` (Setup section), `CLAUDE.md` (status, layout, commands)

**Interfaces:**
- Consumes: the MCP tool names from Task 12. In Claude Code they appear as `mcp__learning-tool__<name>`.

- [ ] **Step 1: Write the three SKILL.md files.** Each has frontmatter `name` and a `description` that triggers correctly:
  - study: "Run Robin's daily spaced-repetition study session…"
  - build-topic: "Build or extend a learning topic from references…"
  - refine-questions: "Improve the question-writing guide from usefulness ratings…"

  Each body implements its flow from the spec's "Skills" section, step by step, naming the exact MCP tools. Must-haves:
  - **study:**
    - never reveal the rubric or answer before Robin attempts the question
    - hints count toward `hints_used`
    - grade each rubric point honestly, accepting equivalent wording
    - inline ratings ("rate N …") recorded with an inferred classification, and never solicited every question
    - offer a rewrite for leeches (via `update_question`)
    - one-line greeting and a short end summary
  - **build-topic:**
    - goal and target level → outline broad to specific → references (WebSearch/WebFetch; download into a temp dir; `pdftotext -layout in.pdf out.txt`, `ocrmypdf` first when `pdftotext` yields almost no text; save web articles as Markdown) → `add_reference` with a summary → `get_guide` → generate questions per subtopic
    - prioritized questions (L0 first), each with a rubric, a `pass_threshold`, a citation, and the `guide_version_id`
    - cross-links
    - default `add_questions` status `active`, with an opt-in review mode
    - report the load forecast
  - **refine-questions:** the spec's five steps, with Robin's approval before `save_guide_version` and before any question changes.

- [ ] **Step 2: Write `scripts/install.sh`.** It must be safe to run repeatedly:
  - check for `cargo`, `claude`, `pdftotext` (suggest `brew install poppler ocrmypdf` if missing)
  - `cargo install --path crates/cli --locked`
  - `claude mcp add --scope user learning-tool -- learning-tool mcp` (skip if `claude mcp get learning-tool` succeeds)
  - `ln -sfn "$PWD/skills/<name>" ~/.claude/skills/<name>` for each skill
  - print next steps
- [ ] **Step 3: Install** with `brew install poppler ocrmypdf && ./scripts/install.sh`. Expected: completes, and `claude mcp list` shows `learning-tool` connected.
- [ ] **Step 4: Run a manual session in a new Claude Code session.**
  - Run `/build-topic` with a small test subject (for example "Helmholtz resonators", target L1) and one real PDF or article.
  - Expected: topics, a reference, and 5–8 questions are created; `learning-tool stats` shows `remaining_today` > 0.
  - Then run `/study`. Expected: questions are asked without leaking answers, grades are recorded, and the end summary is shown.
  - Then rate one question and run `/refine-questions`. Expected: a new guide version is proposed.
- [ ] **Step 5: Update docs.**
  - README gets a **Setup** section (install script, prerequisites, data directory, backups) and a **Daily use** section (`/study`, `/build-topic`, `/refine-questions`, `learning-tool stats`).
  - CLAUDE.md: change the status to "Milestone 1 implemented", and add a short **Layout** list and **Commands** list (`cargo test --workspace`, `cargo sqlx prepare --workspace` after changing queries, `./scripts/install.sh`).
- [ ] **Step 6: Commit** with `git commit -m "Add study, build-topic and refine-questions skills and installer"`.

---

## Spec coverage check

| Spec section | Task(s) |
|---|---|
| Architecture / data directory / install / time | 1, 2, 12, 13 |
| Topics, links, question links, secondary topics | 3, 5 |
| Questions, rubric, revisions, priority, guide version | 2, 5 |
| Review log, derived state, sessions | 4, 7 |
| Ratings, guide versions, examples | 10 |
| References, chunking, FTS | 9 |
| Projects (room only) | 1 (tables only) |
| Settings | 1 |
| FSRS, session building, requeue, leeches, grade mapping | 2, 4, 6, 7 |
| MCP tools, admin CLI, streak | 8, 12 |
| Automatic backups (daily + pre-migration, keep 2, configurable dir), restore | 11, 12 |
| Skills, seed guide | 10, 13 |
| Testing and dogfood acceptance | every task, 12, 13 |
