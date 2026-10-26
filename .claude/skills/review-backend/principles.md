# Backend principles (general)

Applies to any Rust backend. Add entries as you learn; never reuse an ID.

### BE-G1 — Dependencies point inward
**Rule:** Domain logic must not depend on delivery mechanisms (CLI, HTTP, MCP, UI); outer layers depend on the core, never the reverse.
**Why:** The core stays testable and reusable, and new front doors (a web API next to the MCP server) don't require touching it.
**Violation looks like:** A core module importing `rmcp`, `axum`, `clap` or printing for a user; domain types shaped around one transport.
**Typical severity:** high

### BE-G2 — Side effects at the edges
**Rule:** Keep I/O (clock, filesystem, network, environment, randomness) at the boundaries and pass its results in; keep decision logic pure.
**Why:** Pure logic is deterministic, easy to test and easy to reason about; hidden I/O makes behavior depend on when and where it runs.
**Violation looks like:** Reading the clock, env vars or files deep inside business logic; a function that both decides and writes.
**Typical severity:** medium

### BE-G3 — Typed errors at library boundaries
**Rule:** Libraries return specific error types that callers can match on; catch-all error types (`anyhow`) belong in binaries.
**Why:** Callers need to tell "bad input" from "broken system" to respond correctly.
**Violation looks like:** `anyhow::Result` or `Box<dyn Error>` in a library's public API; errors as bare strings; `unwrap()`/`expect()` on input that can actually fail.
**Typical severity:** medium

### BE-G4 — Small, intention-revealing public surface
**Rule:** Expose the fewest public items needed; everything else is `pub(crate)` or private.
**Why:** Every public item is a promise; a small surface keeps refactoring cheap.
**Violation looks like:** Helpers made `pub` "just in case"; internal structs leaking through public signatures.
**Typical severity:** low

### BE-G5 — Make invalid states unrepresentable
**Rule:** Parse and validate at the boundary into types that can only hold valid values (enums, newtypes, constructors), then trust them inside.
**Why:** Validation happens once, and the compiler enforces it everywhere else.
**Violation looks like:** Status or kind fields as free `String`s; the same range check repeated in several places; `Option` pairs that must both be set or both unset.
**Typical severity:** medium

### BE-G6 — Don't block the async runtime
**Rule:** No blocking I/O, long CPU work or blocking locks on async executor threads; use async APIs or `spawn_blocking`.
**Why:** One blocked task stalls every other task on that thread.
**Violation looks like:** `std::fs`/`std::thread::sleep`/heavy computation inside `async fn` on hot paths; `std::sync::Mutex` held across `.await`.
**Typical severity:** medium (high if on a request path that can take seconds)

### BE-G7 — Minimize shared mutable state
**Rule:** Prefer passing data and returning results over global or shared mutable state; when sharing is needed, make ownership and locking explicit.
**Why:** Shared mutable state is the main source of ordering bugs and hard-to-test code.
**Violation looks like:** `static mut`, lazy globals holding mutable data, caches without a clear owner.
**Typical severity:** medium

### BE-G8 — Cohesive modules
**Rule:** A module groups things that change together for one reason; split by responsibility, not by technical layer.
**Why:** Changes stay local, and a reader finds related code in one place.
**Violation looks like:** A `utils`/`helpers` grab-bag; one feature's logic scattered across many modules; a module that keeps growing unrelated functions.
**Typical severity:** low

### BE-G9 — Atomic multi-step changes
**Rule:** Operations that must succeed or fail together run in one transaction (or have explicit compensation), and nothing outside it is left half-done on failure.
**Why:** Partial writes corrupt state in ways that are hard to detect later.
**Violation looks like:** Several dependent writes without a transaction; files written before the DB commit with no cleanup on error.
**Typical severity:** high

### BE-G10 — Test at the seams
**Rule:** New behavior comes with tests at the level it's used (public functions, tool boundaries), exercising real dependencies where practical.
**Why:** Tests at seams survive refactoring and catch integration mistakes that unit-level mocks miss.
**Violation looks like:** New public behavior with no test; tests that only assert on mocks; tests coupled to private internals.
**Typical severity:** medium

### BE-G11 — Rust tests at the right level
**Rule:** Unit tests sit in a `#[cfg(test)] mod tests` beside the code; integration tests in `tests/` go through the public API only, with shared helpers in `tests/common/mod.rs`. Logic lives in a library so tests can reach it. Property tests are welcome for pure functions with wide input spaces.
**Why:** The Rust Book's layout keeps fast, focused tests next to the code and keeps integration tests honest about the public surface.
**Violation looks like:** Logic only reachable through `main`; integration tests reaching into internals through test-only `pub` items; a helper file in `tests/` compiled as an empty test crate.
**Typical severity:** low
