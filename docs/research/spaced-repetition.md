# Spaced repetition: what works

A reference for design decisions. It collects lessons from Anki/SuperMemo practice, the FSRS scheduler, and long-time practitioners, and says what each one implies for this app. Compiled October 2026. Sources are listed at the end.

## Scheduling

**Use FSRS, not SM-2.** FSRS (Free Spaced Repetition Scheduler) is the scheduler in modern Anki and is available as a Rust crate (`fsrs-rs`, maintained by the open-spaced-repetition project). It models each item with:
- *stability*: how many days until recall probability falls to 90%
- *difficulty*: how hard the item is for this learner
- *retrievability*: the current probability of recall, derived from stability and time since last review

It schedules the next review for when retrievability drops to the *desired retention*. SM-2 (classic Anki) uses an "ease factor" that only shrinks on failure. That leads to "ease hell": cards stuck at short intervals forever.

- *Implication:* store raw review events (timestamp, grade, elapsed days). FSRS state can then be recomputed from the log at any time, including "as of" a past date for map playback. FSRS parameters can also be re-optimized from your own history once there are enough reviews (roughly 1000+).
- *Implication:* retrievability is the natural "strength" number for the map's colors. Stability is a good secondary signal: it measures how *durable* a memory is, not just how fresh.

**Desired retention is the main workload knob.** 90% is the default. Raising it toward 95%+ makes the workload grow steeply, because forgetting is exponential, so holding recall near-perfect requires very short intervals. Lowering it toward 80% cuts reviews substantially at the cost of more lapses. FSRS can compute the retention that minimizes study time per unit of knowledge retained.

- *Implication:* use different desired retention for the main program (around 0.9, for the long term) and for project study (lower, and/or with a cap on the maximum interval, since a fade after the project is acceptable).

**Grades.** Four buttons: again / hard / good / easy. A common Anki failure is pressing "hard" when the answer was actually wrong. "Hard" means *recalled, with effort*. Practitioners often recommend treating it as pass/fail: again vs. good, with hard and easy as rare signals.

- *Implication:* Claude grades against a rubric and maps the score to a grade deterministically. That removes the learner's self-grading bias, which is a real advantage over Anki. The mapping should be documented and tunable.

## Workload and burnout

**Limit new items, never reviews.** Each new card generates several reviews in its first month and keeps generating them for years. Burnout almost always comes from adding too much, not from reviewing. The standard advice:
- set a modest new-cards-per-day limit (often 5–20 for serious material)
- do not cap reviews, because a capped review queue silently becomes a backlog

When the review count gets high, pause new cards until it drops.

- *Implication:* a daily new-question cap for the main program, plus an automatic "reviews are high, holding new questions" rule. Show the forecast load ("adding these 30 questions ≈ +N reviews/day next month") when Claude generates a batch.

**Recover backlogs gradually.** After a break, clearing hundreds of overdue reviews in one sitting is demoralizing. Spread them over several days, most-at-risk first.

- *Implication:* the daily session after a gap caps itself and orders by lowest retrievability. The streak UI should forgive gaps rather than punish them (see the motivation principles in the README).

**Leeches.** Items failed repeatedly (Anki's default threshold is 8 lapses) waste a large share of time. The fix is almost never "review more". It's to rewrite the item: make it simpler, add context, or split it.

- *Implication:* detect leeches and have Claude propose a rewrite during the session, instead of silently suspending them.

## Writing good items

From Piotr Wozniak's *Twenty Rules of Formulating Knowledge* (SuperMemo), Andy Matuschak's *How to write good prompts*, and Michael Nielsen's *Augmenting Long-term Memory*:

- **Understand before memorizing.** Memorizing what you don't understand produces brittle items that become leeches.
- **Build on the basics.** Simple, foundational items first; detail later.
- **Minimum information principle.** Each item tests one thing. Simple items are remembered better and scheduled more accurately than compound ones. A complex idea becomes several small items.
- **Avoid sets and enumerations.** "List the 7 types of X" is a bad item. Break it into individual items, or into ordered cloze steps.
- **Precise, unambiguous prompts.** There should be one clear answer, so that recall is consistent from review to review.
- **Effortful retrieval.** Items should require real recall, not recognition. Avoid cues that give the answer away.
- **Redundancy is fine.** Hitting one idea from several angles (definition, application, contrast with a neighbor) builds a more flexible memory.
- **Context cues and sources.** Note where an idea came from, so that a forgotten item can be relearned from its source.
- **Prompt writing is the bottleneck.** Matuschak: an SRS works only as well as its prompts, and writing good ones is hard and slow. This is the central argument for Claude generating questions.
- **Avoid orphans, prefer purpose.** Nielsen found that his worst experiences came from memorizing material disconnected from anything he was doing. Items tied to a goal or to other knowledge stick better.

- *Implication for an LLM quizzer:* the minimum information principle was partly a workaround for pass/fail self-grading. With rubric grading, a single question can test more, *but* the scheduler still models it as one memory. Keep questions focused. Use rubrics for depth of explanation and application (levels L2/L3), not for bundling unrelated facts.
- *Implication:* Claude can vary a question's wording between reviews to prevent rote memorization of the prompt's surface. Keep the *target* fixed and vary the *surface*, so the schedule stays meaningful.

## Habits that keep people going

- **One big collection, daily.** Nielsen and others recommend one main deck reviewed every day, rather than many decks you have to choose between. Daily sessions should be short enough to never skip.
- **Mix the queue.** Reviewing topics interleaved (not one topic at a time) is better for learning, and it's Anki's default.
- **Delete freely.** Removing items that no longer matter keeps the collection worth reviewing.

## Sources

- Wozniak, *Twenty Rules of Formulating Knowledge*: https://www.supermemo.com/en/archives1990-2015/articles/20rules
- Minimum information principle: https://supermemo.guru/wiki/minimum_information_principle
- FSRS optimal retention: https://github.com/open-spaced-repetition/fsrs4anki/wiki/The-optimal-retention
- FSRS overview: https://domenic.me/fsrs/
- Matuschak, *How to write good prompts*: https://andymatuschak.org/prompts/
- Matuschak, prompt-writing bottleneck: https://notes.andymatuschak.org/z2ySBuuMjHwuewNBi65YmXD
- Nielsen, *Augmenting Long-term Memory*: http://augmentingcognition.com/ltm.html
- Anki workload discussion: https://forums.ankiweb.net/t/most-principled-anki-friendly-way-of-drastically-reducing-daily-anki-load/20090
- Things to avoid with Anki: https://andrewzah.com/p/anki-things-to-avoid
