# Milestone 4: Knowledge Map Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a status-change log and a per-day knowledge timeline to `learning-core`, a `/api/map/timeline` endpoint, and a `/map` page. The page draws a zoomable circle-packing map with OKLCH coloring, a Color by switch, tooltips, and history playback in Growing and Fixed modes.

**Architecture:**
- **Core.** It records every status change in an append-only `status_events` table. `timeline::timeline` replays reviews and status events once, using the same scheduler functions as `/study`, and returns per-node daily series for the whole active tree.
- **Frontend.** Pure, unit-tested modules handle the hierarchy and sizing (`tree.ts`), the layout (`layout.ts`, swappable), colors (`colors.ts`, OKLCH bands) and staleness. React components then render an SVG map animated with `motion`. State lives in URL search params.

**Tech Stack:** The existing Rust stack, plus `d3-hierarchy` 3 and `@types/d3-hierarchy`. `motion` 14 is already in the chosen stack; add it now.

**Spec:** `docs/superpowers/specs/2026-10-24-milestone-4-map-design.md`. Its Goals, Core additions and Frontend sections are normative.

## Global Constraints

- No Claude attribution in commits.
- Core functions take `now`, and handlers stay thin (BE-R1, BE-R2).
- `status_events` is append-only: never UPDATE or DELETE it (it joins DB-R1/DB-R4). Write it in the same transaction as the status change, using `begin_write` (DB-R2).
- Timeline numbers must equal the existing functions' numbers for today: `stats::today` counts and `views` recall.
- The OKLCH bands, as in the spec:
  - **Hue:** red 25° → blue 255° *through purple*, i.e. decreasing through 0/360 (25 → 0 → 330 → 255), never through green
  - **Lightness:** 0.85 → 0.45 on log-days 1 → 90
  - **Chroma:** 0.15 fading to a 0.06 floor
  - **Never studied** is hatched grey.
- Stale thresholds are settings: `stale_after_days` (default 14) and `stale_full_days` (default 60), each 1–365, with full > after.
- API types come from ts-rs and are committed (FE-R4). Every tool ignores `._*` (FE-R8).
- URL search params (`topic`, `day`, `color`, `mode`) are present only when set (FE-R2).

## Review Focus

1. **A user with no reviews yet** opens `/map`. Expected: circles sized by today's questions, all hatched grey, and the playback bar disabled with "No history yet". *Test: Task 6, `empty_history_disables_playback`.*
2. **Topic and question labels too long for their circle.** Expected: the label is hidden or ellipsized when it doesn't fit; the accessible name and tooltip still carry the full text. *Test: Task 5, `label_hidden_when_it_does_not_fit`.*
3. **Zooming into a topic with no studied questions in Growing mode on an early day.** Expected: an empty-state message ("Nothing here yet on <date>"), not a blank canvas. *Test: Task 6, `zoomed_empty_day_shows_message`.*
4. **A `topic` search param for an archived or missing topic** (an old link). Expected: falls back to the top level with a notice. *Test: Task 6, `unknown_zoom_topic_falls_back`.*
5. **The browser tab is backgrounded during playback.** Expected: no runaway timers; on return, playback resumes from where it was. Use rAF with elapsed time, not `setInterval` step counts. *Test: Task 6, `playback_uses_elapsed_time`.*

---

## File Structure

```
crates/core/migrations/0004_status_events.sql
crates/core/src/status_log.rs          # record(), events_for(); used by topics/questions
crates/core/src/timeline.rs            # Timeline, TimelineNode, Series, Snapshot, timeline()
crates/core/src/settings.rs            # + stale_after_days, stale_full_days
crates/core/src/{topics,questions}.rs  # write status events
crates/cli/src/web/routes.rs           # + GET /api/map/timeline
web/src/lib/map/{tree,layout,colors,staleness}.ts (+ .test.ts)
web/src/components/map/{MapView,MapTooltip,MapLegend,MapControls,PlaybackBar}.tsx (+ tests)
web/src/routes/map.tsx
web/src/routes/__root.tsx              # enable Map link
```

---

### Task 1: Status-change log and staleness settings

**Files:**
- Create: `crates/core/migrations/0004_status_events.sql`, `crates/core/src/status_log.rs`
- Modify: `crates/core/src/{lib,topics,questions,settings}.rs`

**Interfaces:**
- **Produces:**
  - `str_enum!(Entity { Topic => "topic", Question => "question" })`
  - `str_enum!(StatusReason { Archive => "archive", Unarchive => "unarchive", Suspend => "suspend", Unsuspend => "unsuspend", Retire => "retire", Reword => "reword", Reactivate => "reactivate" })`
  - `pub struct StatusEvent { pub entity: Entity, pub entity_id: i64, pub from_status: String, pub to_status: String, pub reason: StatusReason, pub at: DateTime<Utc> }`
  - `pub(crate) async fn record(conn: &mut SqliteConnection, e: &StatusEvent) -> Result<()>`
  - `pub async fn all(pool) -> Result<Vec<StatusEvent>>`, ordered `(at, id)`
- **Migration:**
  - the table from the spec, plus an index on `(entity, entity_id, at)`
  - settings rows `stale_after_days='14'` and `stale_full_days='60'`
- **`topics::update`:** when the status changes, run inside `begin_write` (change it from a plain query to a transaction) and record `Archive`/`Unarchive`.
- **`questions::set_status`:** wrap in `begin_write` and record an event when the status actually changes. The reason is:
  - active → suspended: `Suspend`
  - suspended → active: `Unsuspend`
  - anything → retired: `Retire`
  - proposed → active: `Reactivate`
- **`questions::update` (rewording):** record `Reword` for the old id (active → retired).
- **`Settings`:**
  - gains `stale_after_days: u32` and `stale_full_days: u32`, validated 1–365
  - `set` rejects `stale_full_days <= stale_after_days` (check against the stored other value)

- [ ] **Step 1: Write failing tests:**
  - `status_log`: `archive_and_unarchive_topic_logged`, `suspend_unsuspend_retire_logged`, `reword_logs_retire_of_old_wording`, `no_event_when_status_unchanged`
  - `rename_only_topic_update_logs_nothing`
  - `settings`: `stale_settings_default_and_validate` (defaults 14/60; "0" and "400" are Invalid; full ≤ after is Invalid)
  - `events_are_never_updated` (grep-style test: it reads `include_str!` of `status_log.rs` and asserts it contains no `UPDATE status_events`/`DELETE FROM status_events`)
- [ ] **Step 2: Run `cargo test -p learning-core status_log`.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `cargo test --workspace`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Log topic and question status changes"`.

---

### Task 2: Timeline in the core

**Files:**
- Create: `crates/core/src/timeline.rs`
- Modify: `crates/core/src/lib.rs`, `crates/core/src/scheduler.rs` (expose a retrievability-at-instant helper if needed)

**Interfaces:**
- **Consumes:** `scheduler::{QState, next, SchedParams, retrievability}`, `Settings::{study_day}`, `status_log::all`.
- **Produces** (all `Serialize`, plus ts-rs behind the `ts` feature):
  - `Timeline`, `TimelineNode`, `Series` as in the spec
  - `str_enum!(NodeKind { Topic => "topic", Question => "question" })`
  - `pub struct Snapshot { pub questions: u32, pub studied: u32, pub recall: Option<f32>, pub durability: Option<f32>, pub last_score: Option<f32>, pub days_since: Option<u32>, pub due: u32 }`
  - `Timeline` also carries `pub stale_after_days: u32, pub stale_full_days: u32` (copied from settings), so the map needs one request.
  - `pub async fn timeline(pool, s: &Settings, now: DateTime<Utc>) -> Result<Timeline>`
- **Algorithm:** follow the spec's steps 1–6.
  - **End-of-day instant** for study day D: the moment the next study day begins (D+1 at `day_rollover_hour`, local to the configured timezone), converted to UTC. For today, use `now`.
  - **Days:**
    - `days` = every calendar day from the first review's study day to today, inclusive
    - with no reviews, `days` is empty and each series is empty, but `now` is still filled
  - **Question nodes:** the newest wording of each revision chain whose topic is in the active frame and whose status today is active or suspended. Its `created_on` is the study day of the chain's first wording's `created_at`.
  - **Existence on day D:**
    - created on or before D, and
    - status on D (from events per the spec's Approximations) is active or suspended
  - **Topic nodes:** active topics whose ancestors are all active.
    - `created_on` = the study day of the topic's `created_at`
    - `questions` = the sum over existing questions in the subtree
    - topics with 0 existing questions keep 0
  - **Aggregation:**
    - means over studied ∧ existing questions
    - `days_since` = the minimum over the subtree
    - `due` counts existing, active (not suspended) questions with `due_on` ≤ D

- [ ] **Step 1: Write failing tests,** using `testutil` plus status events:
  - `empty_db_has_no_days_but_now_filled`
  - `recall_sags_between_reviews_and_jumps_on_review_day` (reviews on day 1 and day 5. Recall on day 4 is below day 1, and on day 5 is above day 4.)
  - `end_of_day_respects_rollover` (a review at 02:00 local counts toward the previous day's series)
  - `revisions_collapse_into_newest_wording` (one node, and it carries the old wording's review on its day)
  - `growing_counts_follow_created_on`
  - `retired_question_counts_before_not_after`
  - `suspended_exists_but_not_due`
  - `topic_aggregates_sum_mean_min`
  - `last_series_value_equals_now`
  - `now_matches_today_stats_and_views` (`now.questions` = active questions per `views::topic_page`; recall equals `TopicCard.avg_retrievability` within 1e-3)
  - `archived_topics_excluded`
  - `perf_5000_questions_3_years` (`#[ignore]`: synthetic data; assert under 300 ms in release)
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.** Keep the replay in plain Rust over loaded vectors. No per-day SQL.
- [ ] **Step 4: Run `cargo test -p learning-core` and `cargo test -p learning-core --release -- --ignored perf_5000`.** Expected: PASS, and under 300 ms.
- [ ] **Step 5: Commit** with `git commit -m "Compute the per-day knowledge timeline"`.

---

### Task 3: Timeline endpoint and types

**Files:**
- Modify: `crates/cli/src/web/routes.rs`, `crates/cli/tests/web_api.rs`
- Regenerate: `web/src/api/generated/*`

**Interfaces:**
- **Produces:** `GET /api/map/timeline` → `Json<Timeline>`, and ts types `Timeline`, `TimelineNode`, `Series`, `Snapshot` and `NodeKind`.

- [ ] **Step 1: Write failing API test `map_timeline_shape`:** seed a topic, a question and one review. The response has `days` (length ≥ 1), a topic node `t<id>` with `series.questions` the length of `days`, and a question node `q<id>` with `parent` `t<id>`.
- [ ] **Step 2: Run it.** Expected: FAIL (404).
- [ ] **Step 3: Implement the route.** Regenerate types with `cargo test --features learning-core/ts,learning-tool/ts export_bindings`.
- [ ] **Step 4: Run `cargo test --workspace`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Serve the map timeline"`.

---

### Task 4: Map logic (pure TypeScript)

**Files:**
- Create: `web/src/lib/map/{tree,layout,colors,staleness}.ts` and their `.test.ts`
- Modify: `web/package.json` (add `d3-hierarchy`, `@types/d3-hierarchy`, `motion`)

**Interfaces:**
- **Produces:**
  - **`staleness.ts`:** `freshness(daysSince: number | null, after: number, full: number): number`. Returns 1 when `daysSince` ≤ `after` or is null (never studied is handled separately), falls linearly to 0 at `full`, and is clamped.
  - **`colors.ts`:**
    - `type ColorMode = "combined" | "recall" | "durability" | "score" | "staleness"`
    - `type Measures = { studied: number; recall: number | null; durability: number | null; lastScore: number | null; daysSince: number | null }`
    - `export const BANDS = { hueWeak: 25, hueStrong: 255, lightFragile: 0.85, lightDurable: 0.45, durFragileDays: 1, durDurableDays: 90, chromaFresh: 0.15, chromaFloor: 0.06 }`
    - `function hueFor(t: number): number`. **Hue interpolation note:** OKLCH hue 25° (red) → 255° (blue), going *through* 330° (magenta/purple), i.e. *decreasing* through 0/360 (25 → 0 → 360 → 255, a span of 130°), NOT increasing through 140° (green). Implement as `(25 - 130 * t + 360) % 360`.
    - `function colorFor(m: Measures, mode: ColorMode, stale: { after: number; full: number }): { fill: string; text: "light" | "dark"; hatched: boolean }`:
      - **Never studied** (`studied === 0`) → `hatched: true` and the neutral fill `oklch(0.9 0 0)`.
      - **Combined:** hue from `lastScore`, lightness from log-scaled `durability`, and chroma = `floor + (fresh - floor) * freshness`.
      - **Single measures:** hue from that measure, with fixed L = 0.65 and C = 0.13. Staleness maps fresh → blue and stale → red.
      - `text` is `"light"` when L < 0.62.
  - **`tree.ts`:**
    - `type SizeMode = "growing" | "fixed"`
    - `function buildTree(nodes: TimelineNode[], rootId: string | null, dayIndex: number | null, mode: SizeMode): TreeNode | null`
    - `TreeNode = { id, kind, refId, label, value, measures: Measures, children: TreeNode[] }`
    - Measures come from `series[dayIndex]`, or from `now` when `dayIndex` is null.
    - In Growing mode, `value` = 1 for an existing question, and nodes with value 0 are dropped. In Fixed mode, the value comes from `now`.
    - Only direct children of the root are drawn as circles; deeper levels are inside each child, rendered one level at a time.
  - **`layout.ts`:**
    - `type Shape = { id: string; x: number; y: number; r: number }`
    - `type Layout = (root: TreeNode, width: number, height: number) => Shape[]`
    - `export const packLayout: Layout`, using d3 `hierarchy(root).sum(v).sort((a, b) => order(a) - order(b))` plus `pack().size([w, h]).padding(6)`, and returning the root's children only (with their own children's shapes, flagged by depth for inner rings)
    - order = the node's `created_on` (string compare), then id

- [ ] **Step 1: Write failing tests:**
  - `staleness.test.ts`: `fresh_until_after`, `linear_to_full`, `clamped`
  - `colors.test.ts`:
    - `never_studied_is_hatched`
    - `combined_respects_bands` (for a grid of inputs, L stays in [0.45, 0.85] and C ≥ 0.06)
    - `hue_goes_through_purple_not_green` (`hueFor(0.5)` ≈ 320 ± 5; never in 90–200)
    - `stale_lowers_chroma_to_floor_not_zero`
    - `text_contrast_switches`
    - `single_measure_views_fixed_light_and_chroma`
  - `tree.test.ts`:
    - `growing_drops_nonexistent`
    - `fixed_uses_now_sizes`
    - `measures_from_day_index`
    - `zoom_root_children`
    - `unknown_root_returns_null`
  - `layout.test.ts`:
    - `stable_order_between_days` (two days with different sizes give the same id order, sorted by created)
    - `circles_inside_bounds`
- [ ] **Step 2: Run `pnpm --dir web test`.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `pnpm --dir web test && pnpm --dir web check`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add map layout, coloring and sizing logic"`.

---

### Task 5: Map view, tooltip and legend

**Files:**
- Create: `web/src/components/map/{MapView,MapTooltip,MapLegend}.tsx` and their tests

**Interfaces:**
- **Consumes:** `buildTree`, `packLayout`, `colorFor`, and the generated `Timeline` types.
- **Produces:**
  - **`MapView`:** props `{ root: TreeNode; width: number; height: number; color: ColorMode; stale: { after: number; full: number }; dayLabel: string; onZoom(id: string): void; onOpenQuestion(id: number): void }`. Each child renders:
    - a `motion.circle` (animating cx, cy, r and fill)
    - a hatched `<pattern>` fill for never-studied nodes
    - a label (name plus count) only when the text fits `2r` (measure via char width × length)
    - `tabIndex=0`, `role="button"`, and `aria-label` = full label plus summary
    - Enter or click zooms into a topic, or opens a question
  - **`MapTooltip`:** content for a node: questions existing/studied, recall %, durability (days), last score %, last studied (date and days ago), due, and target level.
  - **`MapLegend`:** for the current ColorMode, a hue scale bar, light/deep swatches (Combined), fresh/stale swatches, and a hatched "not studied" swatch.

- [ ] **Step 1: Write failing tests:**
  - `MapView.test.tsx`:
    - `renders_a_circle_per_child_with_accessible_name`
    - `click_topic_calls_onZoom`
    - `enter_on_question_opens_it`
    - `label_hidden_when_it_does_not_fit` (accessible name still full)
    - `never_studied_uses_hatch_pattern`
  - `MapTooltip.test.tsx`: `shows_all_measures_and_dates`
  - `MapLegend.test.tsx`: `legend_changes_with_color_mode`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
  - Use `motion/react`.
  - Since tests run in jsdom, check attributes rather than animation.
  - Apply the SVG `role="button"` lint suppression only if Biome flags it, on the element line with a reason (one rule per comment).
- [ ] **Step 4: Run `pnpm --dir web test && pnpm --dir web check`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add the map view, tooltips and legend"`.

---

### Task 6: Map page, playback and navigation

**Files:**
- Create: `web/src/components/map/{MapControls,PlaybackBar}.tsx`, `web/src/routes/map.tsx` and their tests
- Modify: `web/src/api/queries.ts` (`timelineQuery`), `web/src/routes/__root.tsx` (enable Map)

**Interfaces:**
- **Consumes:** `MapView`, `MapLegend`, `buildTree`, and `/api/map/timeline` (whose `Timeline` carries the stale thresholds; see Task 2).
- **Produces:**
  - **`timelineQuery()`:** query key `["timeline"]`. `useRefreshAll` also invalidates it.
  - **`/map` route:**
    - `validateSearch` → `{ topic?: string; day?: string; color?: ColorMode; mode?: SizeMode }`, each present only when set
    - layout: breadcrumbs (from the node parents), the legend, `MapControls` (Color by select, Growing/Fixed toggle), the `PlaybackBar`, and `MapView` in a resize-observed container
    - unknown `topic` → fall back to top level with an inline notice
    - an empty zoom on a day → "Nothing here yet on <date>"
  - **`PlaybackBar`:** props `{ days: string[]; index: number | null; onIndex(i: number | null): void }`.
    - ◀ / ▶ step, and a range input scrubber
    - Play runs over 5 s using `requestAnimationFrame` and elapsed time (`index = floor(progress * (n - 1))`), stopping at today
    - Pause; Esc stops playback
    - disabled with "No history yet" when `days` is empty
    - shows the date label ("Today" when `index` is null or the last index)
  - **Navbar:** Map becomes a real link.

- [ ] **Step 1: Write failing tests:**
  - `PlaybackBar.test.tsx`:
    - `step_buttons_move_one_day`
    - `play_advances_by_elapsed_time` (fake timers plus mocked `requestAnimationFrame` and `performance.now`)
    - `playback_uses_elapsed_time` (skip ahead 2.5 s gives about the middle index)
    - `empty_history_disables_playback`
  - `map.test.tsx` (the route with mocked fetch):
    - `opens_top_level_circles`
    - `unknown_zoom_topic_falls_back`
    - `zoomed_empty_day_shows_message`
    - `color_by_updates_search`
- [ ] **Step 2: Run them.** Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run `pnpm --dir web test && pnpm --dir web check && pnpm --dir web build`.** Expected: PASS.
- [ ] **Step 5: Commit** with `git commit -m "Add the map page with playback"`.

---

### Task 7: Docs, acceptance, review

**Files:**
- Modify: `README.md` (a Map section), `CLAUDE.md` (status: milestones 1, 3 and 4; layout `web/src/lib/map`), `.claude/skills/review-database/conventions.md` (DB-R1/DB-R4 cover `status_events`), `.claude/skills/review-frontend/conventions.md` (FE-R6 notes `lib/map` and swappable layouts)

- [ ] **Step 1: Docs.** Update README, CLAUDE.md and the conventions.
- [ ] **Step 2: Install and accept.** Run `./scripts/install.sh`, then check against a **copy** of the real database (`LEARNING_TOOL_HOME=<copy> learning-tool serve --no-open --port 4790`) in the browser pane, following the spec's manual acceptance:
  - two top-level circles
  - zoom to Impedance, then to its questions
  - Play shows bubbles appearing and recoloring
  - Fixed mode and each Color by view
  - tooltips match the topic pages
- [ ] **Step 3: Final review.** One fresh whole-branch review, applying the four `/review-*` skills' lists. Fix Critical and Important findings with RED→GREEN tests.
- [ ] **Step 4: Commit** with `git commit -m "Document the map and finish milestone 4"`.

---

## Spec coverage check

| Spec item | Task |
|---|---|
| Status-change log (table, written in-transaction, append-only) | 1 |
| Stale settings | 1 |
| Timeline (algorithm, approximations, performance) | 2 |
| `/api/map/timeline` | 3 |
| OKLCH bands, Color by, hatching, label contrast | 4, 5 |
| Circle packing, stable order, swappable layout | 4 |
| Zoom, breadcrumbs, question leaves, tooltip, legend | 5, 6 |
| Playback (Growing default, Fixed, controls, about 5 s) | 4, 6 |
| URL state, navbar | 6 |
| Testing, acceptance, reviews | every task, 7 |
