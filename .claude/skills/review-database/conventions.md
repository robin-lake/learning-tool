# Database conventions (this repo)

How this codebase does things. Each entry says where it was decided. Never reuse an ID.

### DB-R1 — `reviews` and `status_events` are append-only
**Rule:** Never `UPDATE` or `DELETE` rows in `reviews` or `status_events`; corrections are new rows or derived-state changes. Every topic or question status change writes a `status_events` row in the same transaction.
**Why:** The review log is the source of truth for scheduling, heatmaps and map playback.
**Violation looks like:** Any `UPDATE`/`DELETE` on `reviews` or `status_events`; logic that rewrites past grades; a status change that skips `status_log::record`.
**Typical severity:** high
**Decided:** Milestone 1 design (Design principles, Anti-goals).

### DB-R2 — Write transactions use `db::begin_write`
**Rule:** Transactions that write start with `crate::db::begin_write(pool)` (`BEGIN IMMEDIATE`), not `pool.begin()`.
**Why:** A deferred transaction that reads before writing fails instantly with "database is locked" when another Claude Code session writes.
**Violation looks like:** `pool.begin()` followed by any write.
**Typical severity:** high
**Decided:** Milestone 1 final review (Important #1).

### DB-R3 — Static SQL strings with bound values
**Rule:** SQL is a string literal; values are bound with `.bind()`. Lists are passed as JSON and expanded with `json_each(?)`.
**Why:** sqlx 0.9 rejects dynamic SQL strings, and static strings keep queries greppable and safe.
**Violation looks like:** `format!`-built SQL; `AssertSqlSafe`; building `IN (...)` lists by hand.
**Typical severity:** medium
**Decided:** Milestone 1 (Tasks 3 and 9).

### DB-R4 — Nothing user-facing is hard-deleted
**Rule:** Topics become `archived`, questions `suspended`/`retired`, sources `archived`. Only pure queue rows (`session_items`), the rebuildable cache (`question_state`) and a reference's search chunks (`reference_chunks`, re-derived from its `text.md` when the text is replaced) may be deleted.
**Why:** History and links stay intact for the map and playback.
**Violation looks like:** `DELETE` on topics, questions, sources, ratings, guide versions, links or status events.
**Typical severity:** high
**Decided:** Milestone 1 Global Constraints.

### DB-R5 — Derived tables are rebuildable from the log
**Rule:** `question_state` (and any future derived table) must be reproducible from `reviews` (plus revision lineage) by `scheduler::rebuild_all`.
**Why:** Derived state can be repaired or re-tuned without losing history.
**Violation looks like:** Writing a field to `question_state` that rebuild doesn't recompute.
**Typical severity:** high
**Decided:** Milestone 1 design; Task 4 and final-review rulings.

### DB-R6 — Time formats
**Rule:** Instants are RFC 3339 UTC text (via `types::ts`); study days are `YYYY-MM-DD` text computed by `Settings::study_day`.
**Why:** One format to parse, and study days respect the user's rollover hour and time zone.
**Violation looks like:** Other timestamp formats; deriving a day with `date()` on a UTC string in SQL.
**Typical severity:** medium
**Decided:** Milestone 1 Global Constraints.

### DB-R7 — Every table has `created_at` and `updated_at`
**Rule:** New tables include both columns; updates set `updated_at`. (Join tables and the key/value `settings` table are exempt.)
**Why:** Consistent auditing and future sync/export.
**Violation looks like:** A new entity table without the columns; an `UPDATE` that doesn't touch `updated_at`.
**Typical severity:** low
**Decided:** Milestone 1 design (Data model).

### DB-R8 — One new migration file per schema change
**Rule:** Add `crates/core/migrations/NNNN_description.sql` with the next number; existing files are never edited. The automatic pre-migration backup must keep working.
**Why:** Installed databases replay migrations in order; the backup protects the review log during upgrades.
**Violation looks like:** Edits to `0001_schema.sql`/`0002_settings.sql`; changes to `db::open` that skip the pre-migrate backup.
**Typical severity:** high
**Decided:** Milestone 1 (Task 11).

### DB-R9 — Every query is covered by a test
**Rule:** New or changed SQL is exercised by a unit test against in-memory SQLite (`db::test_pool`).
**Why:** Runtime queries aren't checked at compile time; tests are the safety net.
**Violation looks like:** A new query path with no test reaching it.
**Typical severity:** medium
**Decided:** Milestone 1 (Task 1 ruling: runtime queries instead of compile-time macros).
