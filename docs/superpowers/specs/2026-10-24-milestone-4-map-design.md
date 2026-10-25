# Milestone 4: The knowledge map

Date: 2026-10-24. Status: approved 2026-10-24.

## Intent

A visual map of everything Robin is learning: how big each area is, how well it's known, how durable that knowledge is, and what's going stale. Robin can zoom from the top-level topics down to individual questions, and play the history back to watch the map grow and change color over time.

It builds on the milestone 3 web app. The timeline it introduces, per-topic knowledge state for every day, is also the data the future 3D graph will use.

Builds on: [milestone 3 design](2026-10-24-milestone-3-web-app-design.md), CLAUDE.md, the README principles, the review skills.

## Goals

1. **The Map page (`/map`)**, which also enables the navbar link.
   - The opening view shows **top-level topics only**, as **circles** (circle packing), sized by number of questions.
   - **Click a circle to zoom in** to its subtopics, with an animated transition. Breadcrumbs lead back up. At the bottom level, a topic's **questions are circles** that link to their question pages. A topic with both subtopics and its own questions shows both.
2. **Color: the "Combined" view (default).** Colors are defined in **OKLCH**, and each channel stays inside a band:
   - **Hue = last score** (average of each studied question's most recent rubric score). It runs red (weak), through purple, to blue (strong), interpolated through purple and never through green or yellow.
   - **Lightness = durability** (average FSRS stability, in days). A light-but-colored tint means fragile; a deep-but-not-black shade means durable. Never near white or black.
   - **Saturation = freshness.** It's full when studied within the stale threshold, then fades toward a **floor** (never grey) as days since last study grow.
   - **Never studied** is drawn hatched grey: the one true grey on the map.
   - **Labels** switch between dark and light text based on the fill's lightness.
3. **A "Color by" switch: Combined / Recall now / Durability / Last score / Staleness.** Each single-measure view uses the same red→blue hue scale, at fixed lightness and saturation. The switch is on trial: it may be removed if it doesn't earn its place.
4. **A tooltip** on every circle (hover or keyboard focus), for the selected day:
   - questions (existing / studied)
   - average recall
   - average durability (days)
   - average last score
   - last studied (date, and how many days ago)
   - questions due
   - target level (topics only)

   Summary stats on the circles themselves are **left open**: Phase 1 shows name and question count only; whether more is added gets decided by using it.
5. **History playback.**
   - Controls: a date label, ◀ / ▶ to step one study day, a scrubber, and ▶ Play / ❚❚ Pause.
   - Play runs from the first study day to today in about 5 seconds, so longer histories go faster per day.
   - A past day shows the state at the **end of that day** (after its last session; the 4:00 rollover applies). Today shows the current state.
   - **Default mode: Growing.** Circle sizes are the questions that *existed* by that day. A topic's circle appears when it gets its first question, and bubbles grow over time.
   - **Fixed mode** is kept available: today's map with today's sizes, recolored per day, with not-yet-studied items grey. Both modes use the same data and the same rules; only sizing differs.
   - Zooming works during playback.
6. **An exact history going forward:** a new append-only **status-change log** records archive, unarchive, suspend, unsuspend, retire and reword events, so past days know what existed.

## Non-goals

- The 3D graph (milestone 6) and projects (milestone 5).
- A rectangle/treemap UI. The architecture must support it as a small swap (see Layout), but it isn't built now.
- Phone layout polish. It must work in a normal desktop window.
- Perfect history before the status log exists (see Approximations).

## Anti-goals

- **The map disagrees with `/study` or the question pages** about any number. Everything must come from the same core functions.
- **Jittery playback.** Circles must not jump around between days.
- **Colors that can't be read:** washed-out, muddy or illegible labels, or color as the only signal.
- **A slow map.** It opens in well under a second for a year of study.

## Approaches considered

**Timeline computation.**
- **A. Computed on request in the core. Chosen.** One endpoint replays the review and status logs once and returns per-node daily series; the browser animates locally. It's always exact, has no duplicate state, and reuses the FSRS code.
- **B. A stored daily snapshot table.** Rejected: derived state that must be kept in sync and rebuilt whenever the rules change (DB-R5).
- **C. Computing in the browser.** Rejected: it duplicates the scheduling rules in TypeScript (FE-R7).

**Layout.**
- **Circle packing. Chosen.** It's built so a treemap is a small swap.
- Treemap: deferred.

## Core additions

### Status-change log (migration `0004_status_events.sql`)

- **Table** `status_events(id, entity CHECK IN ('topic','question'), entity_id, from_status, to_status, reason CHECK IN ('archive','unarchive','suspend','unsuspend','retire','reword','reactivate'), at, created_at)`. It's append-only, like `reviews`, and indexed on `(entity, entity_id, at)`.
- **Written in the same transaction as every status change:**
  - `topics::update` when the status changes
  - `questions::set_status`
  - `questions::update` when rewording retires the old wording (reason `reword`)
- No backfill.
- DB-R1/DB-R4 gain this table: never update or delete it.

### Timeline (`crates/core/src/timeline.rs`)

```
pub struct Timeline {
    pub days: Vec<NaiveDate>,            // first study day ..= today (empty if never studied)
    pub today: NaiveDate,
    pub nodes: Vec<TimelineNode>,        // whole active tree: topics and question leaves
}
pub struct TimelineNode {
    pub id: String,                      // "t12" or "q34": unique across kinds
    pub kind: NodeKind,                  // Topic | Question
    pub ref_id: i64,
    pub parent: Option<String>,          // topic node id
    pub label: String,                   // topic name / question prompt
    pub target_level: Option<u8>,
    pub created_on: NaiveDate,           // study day it came into existence
    pub series: Series,                  // one entry per `days` element
    pub now: Snapshot,                   // state right now (today's live values)
}
pub struct Series {
    pub questions: Vec<u32>,             // questions existing (topics: subtree; question: 0/1)
    pub studied: Vec<u32>,
    pub recall: Vec<Option<f32>>,        // average over studied, existing questions
    pub durability: Vec<Option<f32>>,    // average stability in days
    pub last_score: Vec<Option<f32>>,
    pub days_since: Vec<Option<u32>>,    // since the most recent review in the subtree
    pub due: Vec<u32>,
}
```

`pub async fn timeline(pool, s: &Settings, now) -> Result<Timeline>`, algorithm:

1. **Load in bulk:**
   - **reviews**, with their session study day, grade, score and question id, ordered by `(reviewed_at, id)`
   - **questions**: id, topic, `revision_of`, `created_at`, status
   - **topics**: id, parent, status, `created_at`, target level
   - **status events**
2. **Collapse revisions.** Each revision chain is represented by its **newest** wording, and the reviews of all its wordings apply to that node. Its `created_on` is the first wording's creation day.
3. **The frame is today's active tree:** active topics whose ancestors are all active, and the questions under them that are active or suspended today. Retired questions that aren't the newest wording of a chain are folded into their chain.
4. **For each question node, step through `days`**, keeping a running `QState`:
   - Apply that day's reviews with `scheduler::next`, using the same parameters as `/study`.
   - Track last score, last study day and `due_on`.
   - At the **end-of-day instant** (the next day's rollover in the configured timezone; for today, `now`), record:
     - `exists`: created on or before the day, and active per the status events (see Approximations)
     - `studied`: any review so far
     - `recall`: retrievability at that instant
     - `durability`: stability
     - `last_score`
     - `days_since`
     - `due`: whether `due_on` ≤ the day
5. **Aggregate bottom-up into topics:**
   - **sums** for questions, studied and due
   - **means over studied, existing questions** for recall, durability and last score (`None` if none)
   - **minimum** of `days_since`
6. **`now`** is today's values, which equal the last element of each series, kept separately for the non-playback view and for the Fixed mode's sizes.

**Approximations** (accepted; Robin said early inconsistencies are fine):
- Before a question or topic has any status events, its status on past days is taken as its current status.
- With events, status on day D is the `to_status` of the last event at or before D's end, or the first event's `from_status` if D is before all events.

**Performance:** target under 300 ms for 5,000 questions over 3 years in a release build. Add a benchmark-style test with synthetic data, marked `#[ignore]` and run on demand.

### API

`GET /api/map/timeline` returns `Timeline` (ts-rs types). Nothing is cached in Phase 1.

## Frontend

- **Route `/map`**, with typed search params, each present only when set:
  - `topic` (the zoom root)
  - `day` (the selected day; absent means today)
  - `color` (`combined` | `recall` | `durability` | `score` | `staleness`)
  - `mode` (`growing` | `fixed`)

  The map is shareable and back/forward works (FE-G5/FE-R2).
- **Pure modules** in `web/src/lib/map/`, unit-tested:
  - **`tree.ts`:** builds the hierarchy from `Timeline.nodes` for a zoom root; picks sizes for a given day index and mode (Growing uses `series.questions[i]`, or 1 per existing question; Fixed uses `now.questions`); nodes of size 0 are omitted in Growing mode.
  - **`layout.ts`:**
    - the layout interface: `type Layout = (root: HierarchyInput, width: number, height: number) => Shape[]`, where `Shape = { id, x, y, r } | { id, x, y, w, h }`
    - `packLayout`, using `d3-hierarchy`'s `pack` with **stable ordering** (children sorted by `created_on`, then id), so playback only moves circles smoothly
    - a treemap later is another `Layout`
  - **`colors.ts`:** `colorFor(measures, mode, settings) → { fill: "oklch(L C H)", text: "light" | "dark", hatched: boolean }`, with the banded channels:
    - **hue:** red ≈ 25° → purple → blue ≈ 255°, from score 0 → 1
    - **lightness:** 0.85 (fragile, durability ≤ 1 day) → 0.45 (durable, durability ≥ 90 days), on a log scale
    - **chroma:** full ≈ 0.15 when fresh, fading to a floor of 0.06 by `stale_full_days`
  - **`staleness.ts`:** freshness from `days_since` using the new settings `stale_after_days` (default 14) and `stale_full_days` (default 60).
- **Components:**
  - **`MapView`:** an SVG canvas sized to its container. Circles animate position, size and color with `motion`, and labels appear only when they fit.
  - **`MapControls`:** Color by, mode, and the playback bar.
  - **`MapTooltip`:** shadcn Tooltip with the goal 4 contents.
  - **`MapLegend`:** explains the hue, depth, fading and hatching.
- **Interaction:**
  - Circles are focusable and Enter zooms in. Question circles link to `/questions/$id`.
  - Escape or the breadcrumbs zoom out.
  - The playback bar is keyboard-operable.
- **New dependencies:** `d3-hierarchy` (already chosen). OKLCH colors are CSS strings, so no color library is needed.
- **Settings:** add `stale_after_days` and `stale_full_days` to the core settings (validated, 1–365, with full > after).

## Calls I made (not discussed; check these)

- **Staleness:** starts at 14 days and reaches the floor at 60 days, both as settings.
- **Color bands:** hue 25° → 255° through purple; lightness 0.85 → 0.45 on a log-days scale (1 → 90 days); chroma 0.15 fading to a 0.06 floor.
- **Playback:** about 5 seconds total, with a scrubber and keyboard control.
- **Question circles** are all equal size at the bottom level.
- **Archived topics** aren't on the map at all (the topic pages still show them).
- **Suspended questions** appear (they exist), but they don't count as due.
- **The timeline covers the whole tree in one response;** zooming happens in the browser without another request.
- **"Recall now"** in the Color by view uses the same retrievability as everywhere else; "Last score" is the mean of each question's most recent review score.
- **Mode and color choices live in the URL** (shareable) rather than in settings.
- **A topic's staleness** is the time since the most recent review *anywhere* in its subtree, so one fresh question keeps a big topic fresh. The alternative is an average across its questions.

## Testing and verification

- **Core:**
  - status events are written for each kind of change, inside the same transaction, and never updated
  - timeline:
    - empty database → no days
    - one question reviewed twice → recall sags between reviews and jumps on review days
    - end-of-day uses the rollover
    - revisions collapse into one node that carries the old wording's reviews
    - Growing counts follow `created_on`
    - a retire event removes a question from later days but not earlier ones
    - topic aggregation (sums, means, minimum `days_since`)
    - the last series value equals `now`, and `now` agrees with `stats::today` and the views' recall
  - an ignored performance test
- **API:** `/api/map/timeline` shape and status codes.
- **Frontend (Vitest):**
  - `colors.ts` band limits: never below the chroma floor, never outside the lightness band, never-studied gives hatched
  - `tree.ts` sizing in both modes, and zero-size omission in Growing
  - `layout.ts` stable order between two days (same order, ids present)
  - `MapView` renders circles with accessible names and opens the tooltip on focus
  - the playback controls step, play, pause and scrub (with fake timers)
  - Color by changes the fills
- **Manual acceptance (against a copy of the real database):**
  - the map opens with two top-level circles
  - zoom to Electric circuits › Impedance, then to its questions
  - play history: bubbles appear and grow, and colors change on the study days
  - switch to Fixed mode and through each Color by view
  - tooltips match the topic pages' numbers
- **Reviews:** run the four `/review-*` skills, then one fresh whole-branch review.

## Left to the builder

- Exact animation timings and easing, the label-fitting rules, and the legend design.
- Component boundaries within `web/src/components/map/`.
- Whether playback steps over every calendar day or only study days (the series include every day).

## Provisional

- The Color by switch, on trial.
- The staleness thresholds and color band values.
- Summary stats on the circles themselves.
