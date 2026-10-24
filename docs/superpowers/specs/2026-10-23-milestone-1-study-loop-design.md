# Milestone 1: Core study loop + topic building

Date: 2026-10-23. Status: approved 2026-10-23.

## Intent

Robin wants a replacement for Anki that builds long-term expertise. It should be a comprehensive, growing map of knowledge, studied every day for years. Claude, running in Claude Code, runs the study sessions. It asks questions, grades free-form answers against rubrics, and helps build topics from good references, so Robin doesn't have to write questions by hand.

Milestone 1 is the smallest version that tests whether the idea works. It covers:
- daily study sessions in Claude Code with rubric grading and FSRS scheduling
- building a real topic, from references through to questions, with Claude's help

There is no web app yet. If after a couple of weeks Robin is studying daily and the generated questions are worth answering, the idea works.

Background reading, which this design follows: [README.md](../../../README.md) (principles from *Ultralearning*, *Make It Stick*, and *The Procrastination Equation*) and [docs/research/spaced-repetition.md](../../research/spaced-repetition.md) (SRS practice).

## Goals

1. `/study` runs a daily main-program session of about 10 questions (about 15–20 minutes). Questions are interleaved across topics, graded against rubrics, and scheduled with FSRS. Every review is stored.
2. `/build-topic` takes a subject (for example "folded horn physics") from nothing to an organized topic. It proposes subtopics, finds references and stores their text, and generates prioritized questions with rubrics at levels L0–L3.
3. Any question can be rated 1–5 for usefulness, with optional feedback. `/refine-questions` turns accumulated ratings into an improved, versioned question-writing guide that `/build-topic` uses.
4. The data model is complete enough that milestones 3–6 (web app, map with history playback, projects, 3D graph) need no destructive schema changes. Those milestones only add data on top.

## Non-goals (milestone 1)

- Any web UI or HTTP API (`serve` comes in milestone 3).
- The projects workflow: `/study-project`, project creation, promote/archive. The schema has room for it, but no tools or skills are built.
- Optimizing FSRS parameters from personal history (it needs about 1000 reviews; added later).
- Importing from Anki.
- Multiple users, sync, or cloud anything.

## Anti-goals (failure even if it technically works)

- **Studying becomes a chore.** Sessions that overrun, backlogs that ambush, or friction at the start all count as failure. (Procrastination Equation: protect expectancy, minimize impulsiveness.)
- **The main program floods.** Generated questions pile up faster than they can be retained.
- **Claude gives away answers.** The rubric or reference answer leaks before Robin attempts the question, which ruins retrieval.
- **History is lost.** The review log is the foundation for every later visualization. Nothing in it is ever overwritten or hard-deleted.
- **Grading feels arbitrary.** Grades must be explainable from the rubric.

## Constraints

- Rust backend, local SQLite, single user on macOS.
- Claude Code is the only client in milestone 1.
- The dependencies are already chosen (see CLAUDE.md): `tokio`, `rmcp` (stdio), `sqlx` (SQLite + migrations + FTS5), `fsrs`, `chrono`, `clap`, `serde`/`serde_json`, `thiserror`/`anyhow`, `tracing`, `ts-rs` (types only needed from milestone 3).
- External tools the skills call, not Rust: `pdftotext` (poppler) and `ocrmypdf` for scanned PDFs.
- No `Co-Authored-By: Claude` lines in commits.

## Approaches considered

**How Claude Code talks to the database.**
- **MCP server + small admin CLI. Chosen.** Typed, validated tools suit long structured payloads (questions, rubrics, reference notes) and make grade writes safe.
- CLI + skills only. Less code, but long text and JSON in shell arguments is fragile, validation is weaker, and there would be many Bash permission prompts.
- HTTP API called with `curl`. It needs a running server and has the same quoting problems. Dropped.

**Database access.**
- **`sqlx`. Chosen.** Migrations are built in, and queries are checked against the schema at compile time.
- `rusqlite` + `refinery`. Simpler, but without the checks.

**Frontend (later milestones).**
- **React + TypeScript. Chosen**, because the goal is a tool to use, not a Rust-frontend experiment.
- Rust/WASM (Leptos). Kept as an option only for the 3D graph module.

## Architecture

```
learning-tool/
  Cargo.toml                 # workspace
  crates/
    core/                    # lib: db, migrations, models, scheduler, session logic, references, guide
    cli/                     # bin "learning-tool": subcommands mcp | migrate | stats | backup
  skills/                    # Claude Code skills (source of truth, symlinked into ~/.claude/skills)
    study/SKILL.md
    build-topic/SKILL.md
    refine-questions/SKILL.md
  docs/
```

- **Data directory:** `~/.learning-tool/`, overridable with `LEARNING_TOOL_HOME`.
  - `learning.db`: SQLite, WAL mode.
  - `references/<reference_id>/`: the original file (`original.pdf`, `original.html`, …) and `text.md`, the extracted text.
  - `backups/`: output of `learning-tool backup`.

  The data lives outside the repo, so a `git clean` can never destroy history.
- **Install:** `cargo install --path crates/cli`. Register the MCP server at user scope (`claude mcp add --scope user learning-tool -- learning-tool mcp`). Symlink `skills/*` into `~/.claude/skills/` so `/study` works from any directory. A `just install` or script does all three (builder's choice of mechanism).
- **The `mcp` subcommand** runs migrations on startup, takes an automatic backup if none exists for today, keeping the 2 most recent (via `VACUUM INTO`, to protect the review log), then serves tools over stdio. All logging goes to stderr or `~/.learning-tool/logs/`, never stdout.
- **Time:** timestamps are stored as UTC (ISO-8601 text). The "study day" is local time with a **4:00 rollover**, as in Anki, so late-night study counts toward the previous day. The rollover hour and the time zone are settings.

## Data model

All tables use `INTEGER PRIMARY KEY` ids plus `created_at` and `updated_at`. Nothing is hard-deleted: rows move to `archived` or `retired` status. JSON columns are validated in Rust.

### Topics and links

**`topics`**: the hierarchy.
- `parent_id` (nullable; null = top level), `name`, `slug` (unique among siblings), `description`.
- `scope`: `main` | `project`. `project_id` (nullable, required when scope = project).
- `target_level`: 0–3, the long-term target depth.
- `status`: `active` | `archived`. `position`: order among siblings.

**`topic_links`**: weighted cross-links, the graph that the map and the 3D view will draw.
- `from_topic_id`, `to_topic_id`, `weight` (0–1, how closely related).
- `kind`: `related` (undirected; store with `from < to`) | `prerequisite` (directed: from must come first) | `applies` (from is used in to).
- `note`.

**`question_links`**: the same shape, between questions. Claude creates these when generating elaboration questions that connect ideas.

**`question_topics`**: optional secondary topics for a question (`question_id`, `topic_id`). The primary topic stays on the question.

### Questions

**`questions`**
- `topic_id` (primary topic) and `level`: 0–3 (the L0–L3 definitions are in CLAUDE.md).
- `prompt`: the canonical question. `answer`: the reference answer, which Robin sees only after attempting.
- `rubric`: JSON `[{ "id": "r1", "point": "…", "weight": 1, "essential": false }]`. An L0 question may have one point, and harder questions have more.
- `pass_threshold`: 0–1, the share of rubric weight needed to pass (default 0.6; harder questions are usually set higher).
- `reference_id` (nullable) and `reference_locator` (for example "p. 47", "§3.2").
- `scope` / `project_id`, as on topics. A question inherits its topic's scope unless set otherwise.
- `status`: `proposed` | `active` | `suspended` | `retired`. Only `active` questions are scheduled.
- `priority`: integer, lower = introduce sooner. `/build-topic` sets it so broad questions come before specific ones.
- `guide_version_id`: the question-writing guide version that produced the question (null if hand-written).
- `revision_of` (nullable): when a question is rewritten, the old row is retired and the new row points back to it, so history stays attached to the exact wording that was reviewed.

### Review log and derived state

**`reviews`**: append-only. This is the source of truth.
- `question_id`, `session_id`, `reviewed_at`.
- `grade`: 1 again, 2 hard, 3 good, 4 easy.
- `score`: 0–1, share of rubric weight hit. `rubric_result`: JSON `[{ "id": "r1", "hit": true, "note": "…" }]`.
- `hints_used` (int) and `fluent` (bool, Claude's judgment that the recall was immediate and complete).
- `user_answer`: Robin's answer, verbatim. `asked_prompt`: the wording actually used, which may be a rephrasing. `feedback`: what Claude told Robin.
- `duration_ms` (nullable).

**`question_state`**: a cache derived from `reviews`, recomputable at any time.
- `question_id` (primary key), `stability`, `difficulty`, `due_on` (study-day date), `last_reviewed_at`, `reps`, `lapses`.
- `state`: `new` | `review` | `relearning`.

**`sessions`**
- `kind`: `main` | `project`. `project_id`.
- `started_at`, `ended_at`, `study_day`, `planned_count`.

### Ratings and the question-writing guide

**`question_ratings`**
- `question_id`, `review_id` (nullable), `rated_at`.
- `usefulness`: 1–5. `feedback`: free text.
- `classification`: `question_quality` | `knowledge_value` | `positive` | null. Claude sets this from Robin's words when recording.
- `processed_in_guide_version_id` (nullable): set when `/refine-questions` has used the rating.

**`guide_versions`**: the question-writing guide, versioned.
- `content`: Markdown. It holds per-level criteria, general rules (seeded from the research doc's "Writing good items" section), and do/don't guidance about what is worth asking.
- `change_summary`, `parent_id`. The newest version is the current one.

**`guide_examples`**: links real questions into a guide version as examples.
- `guide_version_id`, `question_id`, `verdict` (`good` | `bad`), `level`, `reason`.

### References

**`references`** (quoted in SQL, since `REFERENCES` is a keyword; or name the table `sources`, builder's call)
- `title`, `authors`, `kind`: `pdf` | `article` | `book` | `docs` | `video` | `other`.
- `url`, `file_path` (inside the data directory), `text_path`.
- `summary`: Claude's notes on the reference's essence and what it's good for.
- `status`, `added_at`.

**`reference_topics`**: many-to-many between references and topics.

**`reference_chunks`**: the extracted text split into searchable pieces.
- `reference_id`, `locator` ("p. 12" or a heading path), `ordinal`, `text`.
- Backed by an FTS5 virtual table for full-text search.

**Chunking:** split `pdftotext` output on form-feed characters (one chunk per page, locator "p. N"). Split Markdown on headings, with the locator taken from the heading path. Overlong chunks are split further. Rust does the chunking; only the text extraction itself happens outside Rust.

### Projects (room only, unused in milestone 1)

- **`projects`:** `name`, `slug`, `description`, `status`, `desired_retention` (default 0.8), `max_interval_days` (nullable).
- **`project_topics`:** `project_id`, `topic_id`, `required_level` (nullable; can exceed the topic's long-term target).

### Settings

`settings` is a key–value table, with defaults inserted by a migration:

| Key | Default |
|---|---|
| `new_per_day` | 3 |
| `new_pause_review_threshold` | 20 |
| `backlog_daily_limit` | 30 |
| `session_target` | 10 |
| `desired_retention_main` | 0.9 |
| `leech_lapses` | 6 |
| `day_rollover_hour` | 4 |
| `timezone` | system |

## Scheduling and session rules

**FSRS.** Use the `fsrs` crate with default parameters. Schedule at day granularity:
- After each review, compute the new memory state and the next interval at the desired retention (main program: `desired_retention_main`; project questions: the project's value, capped at `max_interval_days`).
- Store the result in `question_state`.
- `question_state` must be rebuildable by replaying `reviews` in order. A `learning-tool` admin command does this, and tests verify it.

**Building a main session (`start_session`).** The queue is built in four steps:
1. **Due reviews:** active main-program questions with `due_on <=` today, ordered by lowest current retrievability. All of them are served, *unless* there are more than `backlog_daily_limit`. In that case the lowest-retrievability `backlog_daily_limit` questions are served and the rest carry over (backlog recovery).
2. **New questions:** if there are `new_pause_review_threshold` or more due reviews, no new questions today. Otherwise allow `new_per_day` minus the number already introduced today.
3. **Choosing new questions:** pick by topic order (shallower topics first, then by `position`), then by `level` ascending, then `priority`. That gives broad before specific, and L0 before L3.
4. **Interleave:** mix new questions among the reviews, and avoid two questions from the same topic in a row where possible. Start the session with a few likely successes (high retrievability) before harder questions, to give an early win.

If no reviews are due and new questions are capped, the session is short. That's fine, and the skill says so cheerfully.

**Again within a session.** A question graded *again* is re-asked once near the end of the same session. The re-ask is recorded as its own review.

**Leeches.** When a question's `lapses` reaches `leech_lapses`, `record_review` flags it. The study skill then offers a rewrite: simpler, more context, or split into several questions.

**Grade mapping.** This is deterministic, in core, so grades are explainable and the thresholds can be tuned in one place. Given `score`, `essentials_met`, `hints_used`, `fluent`, and the question's `pass_threshold`:

| Grade | Rule |
|---|---|
| **again** | any essential point missed, or `score < pass_threshold` |
| **easy** | `score == 1.0`, `hints_used == 0`, and `fluent` |
| **hard** | `hints_used > 0`, or `score < pass_threshold + (1 − pass_threshold)/2` |
| **good** | otherwise |

The rules apply top to bottom. The thresholds are provisional and live in one function.

## MCP tools

The names are indicative; exact shapes are left to the builder. All inputs are validated. Each tool returns structured JSON plus a short human-readable summary.

**Study**
- `start_session(kind="main")`: creates a session and returns counts (due, new allowed, backlog carried over, streak) but no questions yet.
- `next_question(session_id)`: returns one question with `prompt`, `level`, topic path, `rubric`, `answer`, `pass_threshold`, reference citation, recent history (last few reviews), and leech status, or null when the session is done. The skill must never show `rubric` or `answer` to Robin before they answer.
- `record_review(session_id, question_id, rubric_result, hints_used, fluent, user_answer, asked_prompt, feedback, duration_ms?)`: computes the score and grade in core, updates state, and returns the grade, the next due date, and the leech flag.
- `end_session(session_id)`: returns a summary (answered, grade breakdown, streak, tomorrow's expected load).
- `rate_question(question_id, usefulness, feedback?, classification?, review_id?)`.

**Topics and questions**
- `list_topics(root_id?, depth?)`: the tree with question counts by level and status, average retrievability, and target level.
- `get_topic(id)`: details, links, references, and questions.
- `create_topic(...)`, `update_topic(...)`, `link_topics(from, to, kind, weight, note?)`.
- `add_questions(topic_id, questions[], status="active")`: inserts in bulk. Returns ids plus a **load forecast**: how many days of new-question capacity the batch represents at `new_per_day`, and the expected extra daily reviews.
- `update_question(...)`: a wording change creates a revision (`revision_of`). `set_question_status(...)`, `link_questions(...)`.
- `get_question(id)`: the question, its full review history, and its ratings.
- `today_stats()`: answered today, remaining today, streak, new introduced today, due tomorrow.

**References**
- `add_reference(title, kind, url?, authors?, source_file?, text_file?, summary?, topic_ids[])`: copies the files into the data directory, chunks and indexes the text, and links the reference to topics.
- `search_references(query, topic_id?, limit?)`: FTS5 search returning chunks with their locators.
- `get_reference(id)` and `read_reference(id, from_locator?, max_chunks?)`.

**Guide**
- `get_guide()`: the current version, with its examples.
- `list_ratings(unprocessed_only=true)`: ratings joined with the question text, level, and the rating's classification.
- `save_guide_version(content, change_summary, examples[], processed_rating_ids[])`.

**Admin CLI:** `migrate`, `stats` (today's numbers in the terminal), `backup` (manual snapshot to `backups/` with a timestamp), `restore <file>`, and `rebuild-state`.

**Streak definition:** consecutive study days, up to today, on which a main session finished all of that day's due reviews, or at least `session_target` questions when fewer were due. If today isn't done yet, the streak still counts through yesterday, so it doesn't look broken at 8am.

## Skills

The skills live in `skills/` and are written as Claude Code SKILL.md files. They hold the workflows and the tone. Rust holds no prompt text.

### `/study`

1. Call `start_session`. Greet Robin in one line: today's count and streak. No preamble.
2. Loop on `next_question`:
   - **Ask** the question. Claude may rephrase the surface wording to avoid rote memorization, keeping the target identical.
   - **Wait.** Robin answers in free form, or asks for a hint (counted in `hints_used`). Claude never reveals the rubric or answer before Robin attempts.
   - **Grade** each rubric point. Then **tell Robin**: points hit, points missed with a correction, the reference answer when useful, and the citation.
   - **Call `record_review`** and state the resulting grade in a few words.
   - If the question is a leech, offer to rewrite it now.
3. **Ratings stay optional and inline.** At any point Robin can say something like "rate 2, too trivial". Claude records it with a classification inferred from their words. Claude never asks for a rating on every question; at most it occasionally invites one when an answer suggests the question was poor.
4. Call `end_session` and give a short summary: grades, streak, tomorrow's load. End there.
5. **Tone:** brisk and encouraging, honest about misses, celebrating streaks and level-ups without fuss.

### `/build-topic <subject>`

1. **Goal.** Ask why Robin wants this and how deep (target level), unless they already said. Decide where it sits: an existing parent, or a new top-level topic. Check `list_topics` for overlaps and plan cross-links.
2. **Metalearning.** Propose a subtopic outline from broad to specific, with a target level for each. Robin adjusts it. Create the topics.
3. **References.** Search the web for high-quality sources (textbooks, well-regarded articles, official docs, best-practice guides), and accept files Robin provides. For each one:
   - Download it to a temporary path.
   - Extract text: `pdftotext -layout` for PDFs, `ocrmypdf` first if the PDF is scanned, and the page saved as Markdown for web articles.
   - Write a short summary of the reference.
   - Call `add_reference`.
4. **Extract and generate.** Read the current guide (`get_guide`). For each subtopic, use `search_references` and `read_reference` to draw out the essential ideas, then draft questions:
   - prioritized broad first, L0 before deeper levels
   - each with a rubric, a pass threshold, and a citation
   - elaboration questions that link ideas across topics, recorded with `link_questions` / `link_topics`

   Draft only the questions that matter most. Not everything needs a question.
5. **Add.** By default, call `add_questions` as `active` and report the load forecast. If Robin asks to review first, show the draft and add only what they approve. Questions can also be saved as `proposed`.
6. **Close** with a summary: topics created, references added, questions added, and how many days of new-question capacity they represent.

### `/refine-questions`

1. Call `list_ratings(unprocessed_only)` and `get_guide`.
2. Classify any ratings that have no classification yet, using Robin's feedback text:
   - `question_quality`: wording, triviality, wrong level.
   - `knowledge_value`: not worth knowing.
   - `positive`.
3. **Propose a new guide version** to Robin, showing what changed and which ratings drove each change:
   - `question_quality` feedback changes how questions are written.
   - `knowledge_value` feedback changes what is chosen for questions.
   - High and low ratings become good and bad examples.
4. **Propose actions on the rated questions themselves:** rewrite (as a revision), retire, or move out of the main program.
5. After Robin approves, call `save_guide_version` and apply the question actions.

### Seed content

- The first guide version is created by migration or seed. It holds the L0–L3 definitions and the item-writing rules from the research doc.
- No topics or questions are seeded. Robin's first `/build-topic` (likely the subwoofer physics) is the real test.

## Testing and verification

- **Core unit tests** against in-memory SQLite:
  - grade mapping, including edge cases at the thresholds
  - FSRS state updates
  - rebuilding `question_state` from `reviews` gives an identical result
  - session building: the new-question cap, the pause above the threshold, backlog limiting, interleaving, broad-first ordering
  - study-day calculation around the 4:00 rollover and around DST changes
  - streak computation
  - chunking of `pdftotext` and Markdown output
  - FTS search
  - revisions keep history attached to the old wording
- **MCP integration tests:** drive the server over stdio and run a full session (start → next → record → end), plus add a reference and search it.
- **Dogfood acceptance:**
  - build one real topic with references through `/build-topic`
  - study daily for a week
  - rate questions, and run `/refine-questions` once

  Milestone 1 is done when this works without hand-editing the database.

## Calls I made (not discussed; check these)

- Data directory `~/.learning-tool/` (outside the repo).
- Automatic backups: one per day when the MCP server starts, plus one before any migration; keep the 2 most recent (Robin's choice). Local only; the backup folder is a setting so it can point at a synced or external location.
- Skills live in the repo and are symlinked into `~/.claude/skills/`, and the MCP server is registered at user scope, so `/study` works from any directory.
- 4:00 study-day rollover, as in Anki.
- Questions graded *again* are re-asked once later in the same session.
- The grade-mapping table, `pass_threshold` default 0.6, leech threshold 6 lapses, backlog limit 30.
- The streak definition.
- `next_question` serves one question at a time, so Claude never holds the whole queue's answers.
- Rewording a question creates a new revision rather than editing in place.
- The first guide version is seeded from the research doc. No topics are seeded.

## Left to the builder

- Module layout inside `core`, exact SQL, index choices, and migration file breakdown.
- Exact MCP tool names, parameter shapes, and error formats, within the semantics above.
- FTS5 tokenizer and options, and the maximum chunk size.
- Install mechanism (script vs. `just`).
- The wording of the skill files, within the flows and tone above.
- The table name for references (`references` quoted, or `sources`).

## Provisional (expect tuning after dogfooding)

- How levels and subtopic nesting interact.
- Grade-mapping thresholds, the `pass_threshold` default, and the leech threshold.
- Default `new_per_day` (3), the pause threshold (20), and the backlog limit (30).
