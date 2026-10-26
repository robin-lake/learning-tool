# Test-driven development in this repo

How code gets written here: test first, then the code. This doc collects the practices and the sources behind them, with what each means for this repo's backend (Rust), database (SQLite through sqlx) and frontend (React with TypeScript). The four review skills check work against it (CODE-G13, CODE-G14, CODE-R7, BE-G11, BE-R9, DB-G11, DB-R10, FE-G11, FE-R10). Written 2026-10-26.

## The cycle

Kent Beck's own description, [Canon TDD](https://newsletter.kentbeck.com/p/canon-tdd), and Martin Fowler's [summary](https://martinfowler.com/bliki/TestDrivenDevelopment.html):

1. **List the tests.** Write down the behaviors the change needs, including existing behavior that must keep working. Behaviors only: how the code is built inside comes later.
2. **Write one test.** A real, runnable test with setup, an action and assertions. Writing it first forces the decision about how the code will be called.
3. **Watch it fail (red),** and check that it fails for the expected reason: the behavior is missing, not a typo or a compile error elsewhere.
4. **Make it pass (green)** with the simplest change. Add any new cases you discover to the list rather than writing them now.
5. **Refactor** while all tests stay green. Fowler names skipping this step as the most common mistake.
6. Repeat until the list is empty.

Mistakes Beck calls out, which are also what a reviewer looks for:
- tests without meaningful assertions, written only for coverage
- weakening or deleting an assertion to get to green
- copying the code's actual output into the expected value, instead of working the answer out independently
- writing the whole test list as tests up front
- refactoring while making a test pass, or abstracting before duplication has proved itself

**Bug fixes** follow the same cycle: first a test that reproduces the bug and fails, then the fix.

## What makes a good test

- **Test behavior, not internals.** Assert on what goes in and comes out at a public seam, so a refactor doesn't break the test ([The Practical Test Pyramid](https://martinfowler.com/articles/practical-test-pyramid.html), Ham Vocke on martinfowler.com).
- **Spend effort where it buys confidence.** Fowler's pyramid (many unit tests, fewer integration tests, very few end-to-end) and Kent C. Dodds's [Testing Trophy](https://kentcdodds.com/blog/the-testing-trophy-and-testing-classifications) (static checks, then mostly integration tests) agree on the core:
  - cheap checks first: types, Biome, clippy
  - realistic tests at the boundaries where things break
  - don't duplicate a case at a higher level once a lower-level test covers it
- **Tests are code.** Give them readable names that state the behavior (`rename_rejects_duplicate_sibling`), one behavior per test, and an arrange-act-assert shape. Avoid logic inside tests.
- **Deterministic.** No real clock, randomness or network: inject them. A flaky test is a bug.

## Backend (Rust)

Sources: [*The Rust Programming Language*, ch. 11 "Test Organization"](https://doc.rust-lang.org/book/ch11-03-test-organization.html); the Practical Test Pyramid above.

- **Unit tests** go in a `#[cfg(test)] mod tests` beside the code and may test private functions. **Integration tests** go in `crates/*/tests/` and see only the public API. Shared helpers go in `tests/common/mod.rs`.
- **Keep binaries thin** (the Book's advice, and BE-R1): logic in the `learning-core` library, where tests can reach it.
- **This repo's seams:**
  - **Core logic:** tests in `crates/core/src`, against `db::test_pool()` (real SQLite in memory, migrated) and the `testutil` builders. Pass `now` explicitly; never read the clock (BE-R2).
  - **MCP tools:** `crates/cli/tests/mcp_stdio.rs` drives the real server over stdio.
  - **HTTP API:** `crates/cli/tests/web_api.rs` calls the axum router in-process.
- **Property tests** (for example the [proptest](https://docs.rs/proptest) crate) suit invariants over many inputs, such as "a higher rubric score never maps to a lower grade" in `grading`. Consider them for pure functions with wide input spaces; they are not required.
- **Performance tests** are `#[ignore]`d and run in release mode on demand, with the limit and the scenario stated in the test.

## Database (SQLite via sqlx)

Sources: Scott Ambler's case for [test-driven database development](https://www.infoq.com/news/tddd-test-driven-database) (InfoQ) and his book *Agile Database Techniques*; the Practical Test Pyramid above; the [`#[sqlx::test]` docs](https://docs.rs/sqlx/latest/sqlx/attr.test.html).

- **Test against a real database, never a mock.** Queries are runtime strings in this repo (no compile-time checking), so a test that runs the SQL is the only check it gets (DB-R9).
- **Build the test schema with the real migrations.** `db::test_pool()` does this, so tests also prove that a fresh install reaches the schema the code expects. sqlx's `#[sqlx::test]` does the same, giving each test a fresh database and optional SQL fixtures.
- **One fresh database per test.** Tests must not depend on order or share rows. Mind that an in-memory SQLite database lives only as long as its connection.
- **Test the schema's rules too.** Ambler extends TDD to the schema itself:
  - a new constraint, `CHECK`, foreign key or unique index gets a test that a violating write is rejected
  - a migration that changes existing data gets a test that seeds rows in the old shape, runs the migration and checks the result
- **Production settings in tests:** the same pragmas as production. `db::test_pool()` turns foreign keys on, like `db::open`, and uses a single connection so the in-memory database is shared.

## Frontend (TypeScript and React)

Sources:
- Testing Library's [guiding principles](https://testing-library.com/docs/guiding-principles/): tests are more trustworthy the more they resemble how people actually use the software
- Kent C. Dodds, [Common mistakes with React Testing Library](https://kentcdodds.com/blog/common-mistakes-with-react-testing-library)
- TkDodo (a TanStack Query maintainer), [Testing React Query](https://tkdodo.eu/blog/testing-react-query)
- the Testing Trophy above

- **Pure logic first.** Layout, colors, date math and parsing live in `web/src/lib/` as pure functions with Vitest unit tests, written test-first like Rust code.
- **Components are tested the way someone uses them:**
  - **Queries:** `screen.getByRole(…, { name })` first; then label or visible text; `getByTestId` only as a last resort; never `container.querySelector` for things a user perceives.
  - **Waiting:** `findBy*` waits for something to appear (not `waitFor` around `getBy*`). `queryBy*` is only for asserting that something is absent.
  - **Assertions:** use jest-dom matchers (`toBeDisabled`, `toHaveAccessibleName`) rather than raw DOM properties.
  - **Behaviour, not internals:** assert on what the user sees and on the URL, never on component state or hook calls.
  - A test that can't find an element by role often points to a real accessibility gap, so fix the markup rather than the query.
- **Server data:**
  - **Mock the network in one place.** In this repo that is `mockFetch` in `web/src/test/render.tsx`; components never call `fetch` (FE-R3).
  - **Isolate the cache.** Each test gets a fresh `QueryClient` with retries off, which `renderWithApp` provides.
- **Routes:** render the real `routeTree` in a memory history, and assert on `router.state.location` for URL state.
- **Time:** Vitest fake timers, or stubbed `requestAnimationFrame` and `performance.now`, instead of real waits.
- **Static checks count:** TypeScript strict mode and Biome catch whole classes of errors before any test runs; keep them clean.

## Mutation testing: do the tests catch bugs?

Line coverage only says code *ran* under a test. Mutation testing checks whether a test would *notice* if the code were wrong:
1. The tool makes small changes to the code, called mutants: `<` becomes `<=`, `+=` becomes `*=`, a match arm is deleted, a function returns a default.
2. It runs the tests against each mutant.
3. A mutant that a test fails on is *caught*. One that survives marks a gap: code that runs under the tests but isn't really checked.

It's the TDD "watch it fail" step, automated for every line. Fowler's [Test Coverage](https://martinfowler.com/bliki/TestCoverage.html) explains why a coverage percentage is a poor target; the surviving mutants are the useful output instead.

**Rust: [`cargo-mutants`](https://mutants.rs)** (install with `cargo install --locked cargo-mutants`). Tried on this repo on 2026-10-26:

| Run | Mutants | Result | Time |
|---|---|---|---|
| `grading.rs` | 52 | 48 caught, 1 missed, 3 didn't compile | 7 min |
| The milestone 4 Rust diff (`--in-diff`) | 116 | 83 caught, 1 timeout (counts as caught), 4 missed, 28 didn't compile | 11 min |
| The whole workspace | 785 | not run (over an hour) | |

What the survivors found:
- **No test of a score exactly at the pass mark** (grading.rs). Pass at 60% must include 60%.
- **The timeline's "due" counts never checked** when anything was due, for questions or topics.
- **A redundant match arm in `questions::set_status`,** which also hid an untested case.

Each became a test or a simplification, and a re-run caught every mutant.

**How to use it on new code:**
- **Run it on the change, not the crate.** Do this before finishing a branch that touches Rust logic:
  ```bash
  git diff main... -- crates > /tmp/change.diff
  cargo mutants --in-diff /tmp/change.diff -j 4
  ```
  For one module: `cargo mutants -f crates/core/src/grading.rs --test-workspace=false`. Run it in the background; it takes minutes.
- **Results** are in `mutants.out/missed.txt`, which git ignores. A missed mutant is either:
  - **a test gap:** write the test that catches it, then re-run just that mutant with `-F '<mutant description>'` to confirm it's caught
  - **an equivalent mutant**, where the change can't alter behaviour, often redundant code: simplify the code, or note why it's harmless
- **Timeouts count as caught** (usually an infinite loop). **Unviable** mutants didn't compile and mean nothing.
- **No percentage target.** The goal is no unexplained survivors in the code you just wrote.

**TypeScript:** [StrykerJS](https://stryker-mutator.io) does the same with a Vitest runner. It hasn't been tried in this repo yet; `web/src/lib/` (map layout, colors, heatmap) would be the place to start.

## How a reviewer checks TDD

A diff can't show the order things were written in, but it shows the evidence:
1. **Every behavior change comes with a test in the same change.** A bug fix includes a test that reproduces the bug.
2. **The new test fails without the change.** When it's cheap, check this:
   - check out the parent commit in a scratch worktree (`git worktree add`)
   - copy in only the new test, run it, and expect it to fail
3. **The test exercises the real seam.** Look for over-mocking, assertions on internals and copied outputs.
4. **Weakened assertions get justified.** A changed expectation in an existing test needs a reason in the commit or the code.
5. **Rust logic was mutation-tested.** For changes to Rust logic, ask whether `cargo mutants --in-diff` was run and its survivors resolved. If not, and the code matters (grading, scheduling, the queue, the timeline), run it, or recommend it.
