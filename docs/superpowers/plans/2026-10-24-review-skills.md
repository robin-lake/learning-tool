# Review Skills Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add four project-scoped architecture review skills (`/review-backend`, `/review-database`, `/review-frontend`, `/review-code`). They share one procedure and keep their principles in separate, easily extended files.

**Architecture:**
- Markdown only, under `.claude/skills/`.
- Each skill has a short `SKILL.md` that points at the shared `_review/procedure.md` and at its own `principles.md` (general) and `conventions.md` (this repo).
- `docs/review-skills.md` is the overview and holds the removable experimental section.

**Tech Stack:** Claude Code skills (Markdown + YAML frontmatter), git.

**Spec:** `docs/superpowers/specs/2026-10-24-review-skills-design.md`

## Global Constraints

- **Entry IDs** follow `<BE|DB|FE|CODE>-<G|R><n>`, and are never reused. Retired entries are marked *retired*, not deleted.
- **Entry fields**, in order:
  - `### <ID> — <name>` (heading)
  - **Rule:**
  - **Why:**
  - **Violation looks like:**
  - **Typical severity:** one of high / medium / low / nit
  - **Example:** optional
- Repo convention entries add a **Decided:** line that names where or why the rule was decided.
- **Severity scale:** High (blocking), Medium, Low, Nit, with the triage meanings from the spec.
- **Default review range**, in order of precedence:
  1. an argument, if given
  2. on a branch: `main...HEAD`
  3. on `main`: `origin/main..HEAD`
  4. otherwise: the last commit

  Uncommitted changes are included only when asked.
- The skills report only and never edit code, except for the agent's post-build self-review rule in the spec.
- No skill depends on the experimental section.
- No Claude attribution in commits.

## Review Focus

1. **A diff with no files in scope.** Expected: a one-line "nothing in scope" and stop, not a review of unrelated files.
2. **Being on `main` with nothing unpushed.** Expected: fall back to the last commit, and say so.
3. **A violation that existed before the diff.** Expected: a "noticed" note, not a finding that blames the diff.
4. **Two lists flag the same line**, for example `review-code` and `review-backend`. Expected: each skill reports from its own lists only. The overlap is acceptable, but no skill duplicates another's entries.
5. **The frontend skill run before any frontend exists.** Expected: "nothing in scope", not invented findings.

Tests for these are covered by the Task 4 dry runs.

---

### Task 1: Shared procedure and overview

**Files:**
- Create: `.claude/skills/_review/procedure.md`, `docs/review-skills.md`

- [ ] **Step 1:** Write `procedure.md`, implementing the spec's "Shared procedure" steps 1–7 verbatim in substance. It covers:
  - the range rules
  - the scope filter, with the "nothing in scope" stop
  - reading enough context
  - checking every entry
  - reporting pre-existing issues as "noticed"
  - the report layout: range line, severity count line, findings grouped high → nit (each with ID + name, `file:line`, what and why, fix, and a reason when graded off-typical), convention candidates, explicit "no findings"
  - the severity table
  - the edit policy
- [ ] **Step 2:** Write `docs/review-skills.md`:
  - what the four skills are, and when to run them (after building new work)
  - how to add a principle or convention: entry format, next ID, retire rather than delete
  - a final section headed `## Experimental: principles as study material` that says the idea is uncertain and that deleting the section removes it
- [ ] **Step 3: Commit** with `git commit -m "Add shared review procedure and review-skills overview"`.

### Task 2: Backend and database skills

**Files:**
- Create: `.claude/skills/review-backend/{SKILL.md,principles.md,conventions.md}`, `.claude/skills/review-database/{SKILL.md,principles.md,conventions.md}`

- [ ] **Step 1: Backend.**
  - **`SKILL.md`:** frontmatter `name: review-backend`, plus a description that triggers on reviewing Rust/backend structure. Scope: `crates/**/*.rs` and `Cargo.toml` files, excluding SQL concerns, which belong to the database skill. Body: read the procedure, `principles.md` and `conventions.md`, then review.
  - **`principles.md`** (BE-G1…, about 10): dependency direction and layering; I/O and side effects at the edges; pure core logic; explicit typed errors at library boundaries, with `anyhow` only in binaries; small, intention-revealing public APIs; parse-don't-validate types at boundaries; no blocking in async; avoid shared mutable state; module cohesion; tests at the seams.
  - **`conventions.md`** (BE-R1…), from milestone 1:
    - logic in `learning-core`, binaries thin
    - `now: DateTime<Utc>` passed in, never read inside core
    - core `Invalid`/`NotFound`/`Conflict` errors map to MCP tool errors
    - stdout reserved for MCP; logs go to stderr or file
    - shared string enums via `str_enum!` in `types.rs`
    - MCP params reuse core input types where possible, documented with doc comments
    - nothing in core holds prompt text; workflows live in skills
- [ ] **Step 2: Database.**
  - **`SKILL.md`:** scope is `crates/core/migrations/*.sql` plus SQL strings in `crates/**/*.rs`.
  - **`principles.md`** (DB-G1…, about 10): constraints in the schema (NOT NULL, CHECK, FK, UNIQUE); migrations forward-only and never edited after release; short transactions; index for the queries you run; normalize, and denormalize only derived data that can be rebuilt; explicit column lists for writes; parameterized queries; store times in UTC; soft delete or archive for user history; one source of truth.
  - **`conventions.md`** (DB-R1…):
    - `reviews` is append-only
    - write transactions use `db::begin_write` (`BEGIN IMMEDIATE`)
    - SQL is static strings with bound values, and `json_each(?)` for lists
    - nothing is hard-deleted; status columns instead
    - derived tables (`question_state`) are rebuildable from the log
    - timestamps are RFC 3339 UTC text and study days are `YYYY-MM-DD`
    - every table has `created_at`/`updated_at`
    - a new migration file per change, and the pre-migrate backup stays in place
    - every query is covered by a test
- [ ] **Step 3: Commit** with `git commit -m "Add backend and database review skills"`.

### Task 3: Frontend and general-code skills

**Files:**
- Create: `.claude/skills/review-frontend/{SKILL.md,principles.md,conventions.md}`, `.claude/skills/review-code/{SKILL.md,principles.md,conventions.md}`

- [ ] **Step 1: Frontend.**
  - **`SKILL.md`:** scope is frontend source (`web/**`, `*.ts`, `*.tsx`, `*.css`). Before any frontend exists this means "nothing in scope".
  - **`principles.md`** (FE-G1…, about 10): server state separate from UI state; colocate state and keep it minimal; derive rather than duplicate; component boundaries by responsibility; containers versus presentational where it helps; keep side effects out of render; accessible semantics; typed API boundary; no prop drilling beyond about 2 levels; consistent loading, error and empty states.
  - **`conventions.md`** (FE-R1…), the chosen stack only:
    - React + TypeScript
    - TanStack Router for routes, with the map zoom path in the URL
    - TanStack Query for server data
    - shadcn/Radix components
    - API types generated by `ts-rs`, never hand-written
    - d3 for layout math only, with React rendering
    - color tokens with light and dark modes

    Each is marked to be expanded once code exists.
- [ ] **Step 2: Code.**
  - **`SKILL.md`:** scope is all changed source files. It covers language-agnostic quality and defers language- or layer-specific concerns to the other skills.
  - **`principles.md`** (CODE-G1…, about 12): intention-revealing names; consistent vocabulary; no magic numbers or strings (named constants at the narrowest useful scope); single responsibility; small functions at one level of abstraction; DRY versus premature abstraction (the rule of three); comments explain why, not what; dead code removed; fail fast with clear messages; least surprise; immutability by default; keep coupling low.
  - **`conventions.md`** (CODE-R1…):
    - match the surrounding code's naming, idiom and comment density
    - Rust naming (snake_case functions, CamelCase types, SCREAMING_SNAKE constants)
    - domain vocabulary: topic, question, review, session, item, source/reference, guide, study day, lapse, leech
    - constants live next to the code that uses them unless they're shared
    - user-facing error messages name the offending value
- [ ] **Step 3: Commit** with `git commit -m "Add frontend and general-code review skills"`.

### Task 4: Dry runs (the tests)

- [ ] **Step 1:** On a throwaway branch `review-dryrun`, commit planted violations:
  - `Utc::now()` inside a core function → BE-R (no hidden clock)
  - a write transaction with `pool.begin()` → DB-R (`begin_write`)
  - an `UPDATE reviews …` → DB-R (append-only)
  - a magic number `86400` repeated in two places → CODE-G (constants)
  - a function named `do_stuff` → CODE-G (names)
- [ ] **Step 2:** Follow `/review-backend`, `/review-database` and `/review-code` on that branch. Expected: each planted violation is reported with the right ID, `file:line` and a sensible severity (append-only = high). `/review-frontend` reports "nothing in scope".
- [ ] **Step 3:** Run the same three on `main` with nothing unpushed. Expected: falls back to the last commit and states it.
- [ ] **Step 4:** Fix any skill text the dry runs show to be unclear, delete the branch (`git branch -D review-dryrun`), and commit any skill fixes with `git commit -m "Clarify review skills after dry run"`.

### Task 5: Wire up

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1:** Add a short **Reviews** section: after building new work, run the relevant `/review-*` skills on the diff, fix high findings, and normally fix medium findings in new code. It points to `docs/review-skills.md`.
- [ ] **Step 2: Commit** with `git commit -m "Point CLAUDE.md at the review skills"`.
