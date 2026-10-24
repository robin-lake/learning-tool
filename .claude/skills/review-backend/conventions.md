# Backend conventions (this repo)

How this codebase does things. Each entry says where it was decided. Never reuse an ID.

### BE-R1 — Logic lives in `learning-core`; binaries stay thin
**Rule:** All domain logic (scheduling, grading, queue building, topics, references, guide, backups) goes in `crates/core` (`learning-core`). `crates/cli` only parses input, calls core and formats output.
**Why:** The MCP server, the CLI and the future web API must share one set of rules.
**Violation looks like:** Business rules, SQL or computed decisions inside `crates/cli/src/mcp/tools.rs` or `main.rs`; the same logic in two binaries.
**Typical severity:** high
**Decided:** Milestone 1 design (Architecture).

### BE-R2 — No hidden clock
**Rule:** Core functions take `now: DateTime<Utc>` as a parameter; only binaries call `Utc::now()`.
**Why:** Deterministic tests (study-day rollover, DST, streaks) and replayable logic.
**Violation looks like:** `Utc::now()`, `Local::now()` or `SystemTime::now()` in `crates/core/src` outside tests. Accepted exceptions: `db::open` (seed and pre-migrate backup timestamps) and `backup::restore` (renamed-file suffix), which run at the edge.
**Typical severity:** medium
**Decided:** Milestone 1 plan (Architecture).

### BE-R3 — Expected errors become readable tool errors
**Rule:** Core returns `Error::Invalid`, `NotFound` or `Conflict` for problems the caller can fix; the MCP layer turns those into tool errors (via `respond`), and only genuine failures become internal errors.
**Why:** Claude can read and correct a tool error; an internal error just fails.
**Violation looks like:** Input validation that surfaces as a database or internal error; new error variants not mapped in `respond`; error messages that don't name the bad value.
**Typical severity:** medium
**Decided:** Milestone 1 (Task 12); review finding on FK errors.

### BE-R4 — stdout belongs to MCP
**Rule:** Nothing in the `mcp` path writes to stdout; logs go through `tracing` to stderr or `logs/`.
**Why:** Any stray stdout byte corrupts the MCP protocol stream.
**Violation looks like:** `println!`/`print!`/`dbg!` reachable from `learning-tool mcp`; a logger writing to stdout.
**Typical severity:** high
**Decided:** Milestone 1 Global Constraints.

### BE-R5 — String enums via `str_enum!`
**Rule:** Enums stored as text in SQLite are declared with `str_enum!` in `types.rs` (or next to their owner), giving `as_str`/`parse` and lowercase serde.
**Why:** One mapping between Rust, SQL and JSON; invalid strings are rejected in one place.
**Violation looks like:** Hand-written `match` on status strings; `String` fields for closed sets of values.
**Typical severity:** low
**Decided:** Milestone 1 (Task 3 ruling).

### BE-R6 — MCP params reuse core input types
**Rule:** Tool parameter structs embed or reuse core input types (`NewTopic`, `NewQuestion`, `ReviewInput`, `NewSource`) instead of duplicating fields; doc comments on fields are the schema Claude reads.
**Why:** One definition of each input; documentation travels with it.
**Violation looks like:** A params struct that copies a core struct's fields; fields without doc comments where the meaning isn't obvious.
**Typical severity:** low
**Decided:** Milestone 1 (Task 12).

### BE-R7 — No prompt text in Rust
**Rule:** Workflows, tone and instructions for Claude live in the skill files (`skills/`); Rust holds data and rules. Short tool descriptions are the exception.
**Why:** Workflows can then be tuned without rebuilding, and the core stays a plain library.
**Violation looks like:** Multi-step instructions or conversational wording in Rust strings; behavior that depends on parsing Claude's prose.
**Typical severity:** medium
**Decided:** Milestone 1 design (Skills).

### BE-R8 — Core state the scheduler derives is rebuildable
**Rule:** Anything derived from the review log (e.g. `question_state`) can be recomputed by `scheduler::rebuild_all`, and new derived state must keep that property.
**Why:** The log is the source of truth; derived state can be repaired or re-tuned.
**Violation looks like:** Storing a derived value that rebuild can't reproduce; rebuild and incremental paths computing it differently.
**Typical severity:** high
**Decided:** Milestone 1 design (Design principles).
