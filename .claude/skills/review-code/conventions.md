# Code conventions (this repo)

How this codebase does things. Each entry says where it was decided. Never reuse an ID.

### CODE-R1 — Match the surrounding code
**Rule:** New code follows the naming, idioms, error-handling style and comment density of the code around it.
**Why:** A codebase that reads as one voice is easier to navigate.
**Violation looks like:** A new style of error handling in one module; heavy commenting in a sparsely commented file, or the reverse.
**Typical severity:** low
**Decided:** Project working agreement.

### CODE-R2 — Language naming conventions
**Rule:**
- Rust: `snake_case` functions, variables and modules; `CamelCase` types; `SCREAMING_SNAKE_CASE` constants.
- TypeScript: `camelCase` values; `PascalCase` components and types.
- SQL: `snake_case` tables and columns, with plural table names.
**Why:** Matches each language's ecosystem and the existing code.
**Violation looks like:** Mixed casing; singular table names; Hungarian prefixes.
**Typical severity:** nit
**Decided:** Milestone 1 code.

### CODE-R3 — Use the domain vocabulary
**Rule:** Use the established terms:
- **topic:** a node in the knowledge tree.
- **question:** with a **level** (L0–L3), **rubric** and **pass threshold**.
- **review:** one graded answer, in the append-only log.
- **session:** one study sitting. Its queue entries are **items**.
- **source**/**reference:** the DB table is `sources`; the tools say "reference".
- **guide:** the question-writing guide, versioned.
- **study day:** the local day with rollover.
- **lapse:** a failed review on a new study day.
- **leech:** a question at or over `leech_lapses` lapses.
- **main program** vs **project** scope.
**Why:** Consistent words across Rust, SQL, tools and skills.
**Violation looks like:** "card", "deck", "attempt", "note" or "day" used for these concepts in new code.
**Typical severity:** low
**Decided:** Milestone 1 design.

### CODE-R4 — Constants live next to their use
**Rule:** A constant sits at the top of the module that uses it (e.g. `WARMUP_MIN_R` in `queue.rs`). Shared ones go in the owning module and are imported. Values Robin may want to tune go in `settings`, not in constants.
**Why:** Easy to find, and tunables can change without a rebuild.
**Violation looks like:** A `constants.rs` grab-bag; a user-tunable threshold hard-coded as a constant; the same constant defined twice.
**Typical severity:** low
**Decided:** Milestone 1 code (`queue.rs`, `questions.rs`, `settings.rs`).

### CODE-R5 — Error messages name the offending value
**Rule:** Validation errors say what was wrong and include the value, id or field (e.g. `rubric point "zz" was not graded`).
**Why:** Claude reads these through MCP tool errors and must be able to fix the call.
**Violation looks like:** `Error::Invalid("bad input".into())`.
**Typical severity:** low
**Decided:** Milestone 1 (MCP error handling).

### CODE-R6 — Comments explain decisions, briefly
**Rule:** Module and public-item doc comments are one or two lines. Inline comments only explain non-obvious reasons (e.g. the DST note in `clock.rs`, the `BEGIN IMMEDIATE` note in `db.rs`).
**Why:** Keeps the code's existing low, purposeful comment density.
**Violation looks like:** Paragraph-long doc comments on simple functions; comments restating the code.
**Typical severity:** nit
**Decided:** Milestone 1 code.

### CODE-R7 — Test-driven development, checked in review
**Rule:** All new code in this repo is written test-first per [`docs/research/testing.md`](../../../docs/research/testing.md). Reviews read the tests in the diff first and, for a bug fix or risky change, confirm the new test fails without it: run it against the parent commit in a scratch worktree (`git worktree add`), never by editing the working tree.
**Why:** Robin asked for TDD in writing and reviewing code; the review is where the evidence gets checked.
**Violation looks like:** A change whose tests came in a later commit, or never; a "test" that cannot fail; skipped (`#[ignore]`, `it.skip`) tests without a stated reason.
**Typical severity:** medium
**Decided:** Robin's request, 2026-10-26.
