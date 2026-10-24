# Architecture review skills

Date: 2026-10-24. Status: approved 2026-10-24.

## Intent

The goal is review skills that check newly committed work for structural problems. The usual case: the agent builds something and then reviews its own diff to catch architectural issues it may have introduced. The skills don't hunt for bugs; the built-in `/code-review` already does that.

Each skill has two parts:
- **General best practices.** This part grows as Robin learns more about software architecture.
- **This repo's conventions.** These rules keep the codebase consistent.

## Skills

| Skill | Covers |
|---|---|
| `/review-backend` | Rust crates: module boundaries, layering, error handling, async, API shape |
| `/review-database` | Schema, migrations, and SQL in code: transactions, constraints, indexes, the append-only log |
| `/review-frontend` | The React app (milestone 3+): components, state, data fetching, routing, styling |
| `/review-code` | Language-agnostic: naming, constants and magic values, function size, duplication, comments, design principles |

## Approaches considered

- **A. Four self-contained skills.** Simplest, but the procedure is copied four times, and the principle lists get mixed into the instructions.
- **B. Four skills with principles in their own files and one shared procedure.** **Chosen.** Growing a list means editing a plain document. The procedure lives in one place.
- **C. One `/review` skill with four rulebooks.** Rejected: Robin wants four distinct skills. A `/review` that runs the relevant skills can be added on top of B later.

## Layout

```
.claude/skills/
  _review/procedure.md            # shared: what to review, how to cite, report format
  review-backend/   SKILL.md  principles.md  conventions.md
  review-database/  SKILL.md  principles.md  conventions.md
  review-frontend/  SKILL.md  principles.md  conventions.md
  review-code/      SKILL.md  principles.md  conventions.md
docs/review-skills.md             # overview, how to extend, the experimental section
```

The skills are project-scoped (`.claude/skills/`), committed, and not symlinked globally the way the study skills are.

- **`SKILL.md`** (short): frontmatter, scope (which files it reviews), and instructions to read the procedure and both lists, then review.
- **`principles.md`**: general entries.
- **`conventions.md`**: repo-specific entries. Every convention records the decision behind it.

## Entry format (both lists)

```
### BE-G3 — Keep I/O at the edges
**Rule:** …one line…
**Why:** …
**Violation looks like:** …
**Typical severity:** medium
**Example (optional):** …
```

`Typical severity` is a starting point. Each finding's severity is judged from its actual impact in the diff (see below).

IDs have the form `<area>-<G|R><n>`:
- Areas: `BE` backend, `DB` database, `FE` frontend, `CODE` general code.
- `G` marks a general principle; `R` marks a repo convention.

IDs are never reused. A retired entry is marked *retired* rather than deleted, so old review reports still resolve.

## Shared procedure

1. **Pick what to review.** In order:
   1. A commit or range passed as an argument.
   2. On a branch other than `main`: `git diff main...HEAD`.
   3. On `main`: unpushed commits (`origin/main..HEAD`).
   4. Otherwise, the last commit.

   State the range at the top of the report. Uncommitted changes are included only if asked.
2. **Filter** to the files in the skill's scope. If none are in scope, say so in one line and stop.
3. **Read** the diff with enough surrounding code to judge structure, not just the changed lines.
4. **Check** every principle and convention in the skill's two lists. Report only real violations introduced or worsened by the diff. Pre-existing issues get at most a short "noticed" line.
5. **Report:**
   - Findings grouped by severity, highest first.
   - Each finding gives:
     - the entry ID and name
     - `file:line`
     - what's wrong and why it matters, in one or two sentences
     - a suggested fix
   - Then **Convention candidates**: patterns the diff repeats or settles that aren't written down yet, proposed as new `R` entries.
   - If there are no findings, say so plainly.
6. **Severity**, judged by what the violation costs if it ships, not by how the entry is worded:

   | Severity | Meaning | Triage |
   |---|---|---|
   | **High (blocking)** | Breaks an invariant or boundary: risk of data loss or corruption, wrong behavior, a security problem, or a structural choice that would be expensive to undo once other code builds on it. | Fix before the work lands. |
   | **Medium** | A real structural cost: harder to change, test, or understand, or it spreads a pattern the codebase is trying to avoid. | Fix in new code; existing instances can wait. |
   | **Low** | A minor deviation with little lasting cost. | Fix if cheap; fine to leave. |
   | **Nit** | Style or taste; no structural effect. | Optional. |

   A finding may be graded above or below the entry's typical severity, with a few words saying why.
   The report opens with a one-line count (e.g. "1 high, 2 medium, 3 low, 1 nit").
7. **Don't edit code** unless asked. If the review runs right after the agent's own build, the agent may fix the findings it agrees with, each in its own commit: always the high ones, normally the medium ones in code it just wrote. It must say which ones it left and why.

## Initial content (my calls; check these)

- **General lists:** each starts with roughly 8–12 well-established principles:
  - backend: layering and dependency direction, I/O at the edges, explicit errors, small public APIs…
  - database: normalization, constraints in the schema, migrations forward-only, short transactions…
  - frontend: separating server state from UI state, colocation, component boundaries, accessibility basics…
  - code: intention-revealing names, no magic numbers, single responsibility, DRY versus premature abstraction, comments explain why…
- **Repo conventions** come from what's already decided:
  - **Backend:** logic in `learning-core` and thin binaries; no hidden clock (`now` is passed in); expected errors become tool errors; logs never go to stdout.
  - **Database:** `reviews` is append-only; write transactions use `begin_write` (`BEGIN IMMEDIATE`); SQL strings are static with bound values; nothing is hard-deleted; derived tables can be rebuilt.
  - **Code:** existing naming idioms; the comment density shown in the code.
  - **Frontend:** only the chosen stack for now (React + TypeScript, TanStack Router and Query, shadcn, API types generated with `ts-rs`, no hand-written API types). It firms up once code exists.
- **CLAUDE.md** gets one line: after building new work, run the relevant `/review-*` skills on the diff.

## Experimental: principles as study material

Possibly, the general lists could also serve as references for a "Software architecture" topic in the learning tool, with principles that keep showing up in findings turned into questions. **It's uncertain whether this helps.** The idea lives only in one section of `docs/review-skills.md`. No skill, procedure, or list depends on it, so removing it means deleting that section.

## Out of scope

- Bug hunting (use `/code-review`).
- CI or hook automation that runs reviews on every commit (possible later).
- An umbrella `/review` command (possible later, on top of this).
