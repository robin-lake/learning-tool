# Learning Tool

A personal spaced-repetition system built for long-term expertise. Claude, running in Claude Code, quizzes you, grades your answers against rubrics, and helps you build topics from good references. A local web app shows your progress: a daily summary, a study heatmap, and a zoomable map of what you know and how well.

It's meant as an Anki replacement with three differences:
- **Questions don't have to be written by hand.** Claude finds references and turns them into questions with you.
- **Grading is richer than pass/fail.** Harder questions have rubrics, and depth is tracked in mastery levels (L0 orientation → L3 expert).
- **Your knowledge is visible as a map**, with topics cross-linked to show how ideas connect.

Status: milestones 1 (study loop + topic building), 3 (web app, part one) and 4 (the knowledge map) implemented. See [CLAUDE.md](CLAUDE.md) for decisions and [docs/research/spaced-repetition.md](docs/research/spaced-repetition.md) for SRS research.

## Setup

Prerequisites: Rust (rustup), Claude Code, pnpm (`brew install pnpm`), and for PDFs `brew install poppler ocrmypdf`.

```bash
./scripts/install.sh
```

This builds the web app, builds and installs the `learning-tool` binary with the web app inside it, registers it with Claude Code as a user-scope MCP server, links the skills into `~/.claude/skills/`, and creates the database. Start a new Claude Code session afterwards.

- **Data** lives in `~/.learning-tool/` (set `LEARNING_TOOL_HOME` to move it): `learning.db`, `references/` (original files and extracted text), `backups/`, `logs/`.
- **Backups:** a snapshot is taken the first time the MCP server starts each day and before any database migration; the 2 most recent are kept. `learning-tool config set backup_dir <path>` points them somewhere else (e.g. a synced folder). Restore with `learning-tool restore <file> --yes` after quitting Claude Code.

## Daily use

- `/study` — today's session: due reviews plus a few new questions, graded against rubrics. Say "rate 2, too trivial" (or any 1–5 with a reason) whenever a question deserves feedback.
- `/build-topic <subject>` — outline a subject, gather references, and generate questions.
- `/refine-questions` — turn your ratings into a better question-writing guide.
- `learning-tool stats` — today's numbers in the terminal. `learning-tool config get` shows settings (new questions per day, retention target, day rollover hour, timezone…).

## Web app

```bash
learning-tool serve
```

This opens your browser at <http://localhost:4747> (Ctrl+C stops it; studying works with or without it running). Use `--port` or `learning-tool config set web_port <n>` to change the port, and `--no-open` to skip the browser.

- **Home:** today's numbers (answered, remaining, streak), a year of daily activity with per-day tooltips (grades and topics), what's coming up (including pinned questions), and what needs attention (leeches, ratings for `/refine-questions`).
- **Topics:** every topic at any depth, with breadcrumbs, subtopic cards and their progress, and the questions in each. Rename, archive or add subtopics here.
- **Questions:** the wording, answer and rubric, the schedule, every attempt you've made (your answers, points hit or missed, Claude's feedback), a 1–5 usefulness rating, and editing. Rewording creates a new version and keeps the history.
- **Map:** every topic as a circle sized by its number of questions. Click a circle to zoom in (breadcrumbs take you back up); at the bottom level the circles are questions and open their pages. Hover or focus a circle for its numbers: recall now, durability, last score, when you last studied it, what's due and the target level.
  - **Colors (Combined):** hue is your last score (red → purple → blue), depth is durability (pale = fragile, deep = memory that lasts months), and colors fade as a topic goes unstudied (from `stale_after_days`, 14, to `stale_full_days`, 60). Hatched grey means never studied. **Color by** shows one measure at a time instead.
  - **History:** step, scrub or play (about 5 seconds from your first review to today). **Growing** shows circles appearing as questions were added; **Fixed** keeps today's sizes and only changes colors. The topic, day and view are in the URL, so a view can be bookmarked.
- **Pins:** pin a question and it's asked first in your next `/study` session. A pin that isn't due uses one of that day's new-question slots; extras wait a day.

Quizzing stays in Claude Code; the web app is for seeing progress and tending the knowledge base.

## Principles from the books

These three books are the app's design foundation. Each summary is followed by what it means for the app. When a design question comes up, check it against these.

### *Ultralearning*: Scott H. Young (2019)

Young describes aggressive, self-directed learning projects, built on nine principles:

1. **Metalearning.** Before starting, map how the subject is learned: what concepts, facts, and procedures it involves, and what resources the experts use. Spend about 10% of a project's time on this up front.
2. **Focus.** Start quickly, sustain concentration, and match the kind of attention to the task.
3. **Directness.** Learn by doing the thing in the context where you'll use it. Transfer from abstract study is weak.
4. **Drill.** Find the weakest component that limits overall performance (the rate-determining step) and isolate it for focused practice.
5. **Retrieval.** Test yourself to learn, not just to measure. Recall from memory, especially without cues, beats review.
6. **Feedback.** Seek fast, accurate, even uncomfortable feedback. Outcome, informational, and corrective feedback each serve a purpose.
7. **Retention.** Fight forgetting with spacing, overlearning core skills, and turning knowledge into procedures.
8. **Intuition.** Dig deep before building up. Explain ideas from first principles (the Feynman technique) and don't settle for surface patterns.
9. **Experimentation.** Explore beyond comfortable methods. Try new approaches and styles once the basics are in place.

**In the app:**
- *Metalearning* is the "build a topic" skill. Claude maps a subject, finds references, and proposes a topic structure and level targets before generating questions.
- *Directness* is why the higher levels (L2/L3) ask you to calculate, design, and decide, not just define. It's also why projects exist.
- *Drill*: the map makes weak spots visible (red, below target). A session can offer to drill the weakest subtopic.
- *Retrieval* and *feedback* are each review. Claude tells you right away which rubric points you hit and missed.
- *Intuition*: deeper questions ask "why" and "explain it as if to a beginner".

### *Make It Stick: The Science of Successful Learning*: Peter C. Brown, Henry L. Roediger III, Mark A. McDaniel (2014)

Research-based learning science, much of it counterintuitive:

- **Retrieval practice beats rereading.** Pulling knowledge from memory strengthens it. Rereading feels productive but mostly builds familiarity.
- **Space it out.** Spaced practice produces longer retention than massed practice (cramming), because some forgetting before re-retrieval makes the retrieval more effortful and more consolidating.
- **Interleave and vary.** Mixing different kinds of problems feels slower but builds the ability to tell *which* approach a problem needs, and transfers better.
- **Desirable difficulties.** Learning that takes effort lasts longer. Difficulty that you can overcome is a feature, not a bug.
- **Generation.** Trying to answer *before* being shown the solution improves learning, even when the attempt is wrong.
- **Elaboration.** Explaining new material in your own words and connecting it to what you already know builds richer, more retrievable memory.
- **Reflection.** Reviewing what happened, what worked, and what you'd do differently combines retrieval and elaboration.
- **Calibration.** People routinely misjudge what they know (illusions of knowing, fluency mistaken for mastery). Use objective tests to correct your judgment.
- **Mental models and structure.** Experts extract the underlying rules and build structures that organize knowledge. That structure helps learning and recall.
- **Abilities grow.** Effortful learning changes the brain. A growth mindset matters, and "learning styles" matching has little support.

**In the app:**
- Every review is *retrieval*: free recall in your own words, never multiple choice by default.
- The daily queue *interleaves* topics rather than studying one at a time.
- Claude's rubric grading is *calibration*: an outside judgment, not your feeling of knowing. The question page can show "how sure were you" vs. "how did you do".
- *Elaboration* and *generation* questions ("how does this relate to X?", "predict what happens if…") are first-class question types. Cross-links between topics come from these connections.
- Rephrasing questions between reviews keeps difficulty desirable and blocks surface memorization.

### *The Procrastination Equation*: Piers Steel (2010)

Steel combines motivation research into one formula:

> Motivation = (Expectancy × Value) / (Impulsiveness × Delay)

You act on a task when you expect to succeed (expectancy) and the task feels rewarding (value). Motivation drops when you're easily distracted (impulsiveness) and the payoff is far away (delay). Each factor can be worked on:

- **Raise expectancy.** Build success spirals of small, reliable wins. Learn from others' successes. Contrast where you are with where you want to be so that goals feel achievable.
- **Raise value.** Make the work more engaging (challenge matched to skill, game-like goals). Tie it to things you care about. Reward progress.
- **Lower impulsiveness.** Precommit (decide in advance, so in-the-moment willpower isn't needed). Control your environment and remove cues for distraction. Make the right action the easy default.
- **Shrink delay.** Break long goals into near-term subgoals with their own deadlines and visible progress. Short, frequent, concrete targets beat distant abstract ones.

**In the app:**
- *Expectancy:* daily sessions are short, finishable, and predictable. "Remaining today" is a number you can actually reach. Sessions open with items you're likely to get right before harder ones, and never ambush you with a backlog of hundreds.
- *Value:* visible progress. Topics turn bluer on the map, the history playback shows growth, and the heatmap fills in. Level-ups (L1 → L2 on a topic) are milestones worth marking.
- *Impulsiveness:* starting is one command (`/study`). No setup and no decisions at the start. The session picks the questions.
- *Delay:* the long goal (expertise in 10 years) is split into today's queue, this week's streak, and a topic's next level.
- Streaks should motivate without punishing. A missed day should never wipe out the sense of progress, because a broken success spiral hurts expectancy.

## Where they agree

All three books converge on a few rules this app is built around:

1. **Retrieval, spaced and interleaved**, is the core study activity.
2. **Effort is the point.** Harder, deeper questions (higher levels, rubrics, calculations) produce durable knowledge.
3. **Accurate feedback, immediately.** An outside grader beats self-assessment.
4. **Structure and connection.** Knowledge organized into topics and linked across them is easier to learn and to see.
5. **Make showing up easy and rewarding.** The best system is the one you use every day.
