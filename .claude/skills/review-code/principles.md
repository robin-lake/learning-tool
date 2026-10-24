# Coding principles (general)

Language-agnostic. Add entries as you learn; never reuse an ID.

### CODE-G1 — Names reveal intent
**Rule:** A name says what something is or does in domain terms, at the level of abstraction where it's used.
**Why:** Code is read far more than it is written; a good name removes the need for a comment.
**Violation looks like:** `data`, `info`, `tmp`, `do_stuff`, `handle`, `x2`; names that describe type instead of meaning; misleading names.
**Typical severity:** low (medium for public APIs)

### CODE-G2 — One word per concept
**Rule:** Use one term consistently for each concept, and different terms for different concepts.
**Why:** Synonyms make readers wonder whether two things differ.
**Violation looks like:** `fetch`/`get`/`load` used interchangeably; `item` meaning two different things.
**Typical severity:** low

### CODE-G3 — No magic numbers or strings
**Rule:** Literal values with meaning get a named constant at the narrowest useful scope, with units in the name when relevant.
**Why:** The name documents intent, and the value can change in one place.
**Violation looks like:** `86400`, `0.85` or `"active"` repeated or unexplained; thresholds inline in conditions.
**Typical severity:** low (medium when duplicated)

### CODE-G4 — Single responsibility
**Rule:** A function or type does one thing; if describing it needs "and", consider splitting.
**Why:** Focused units are easier to test, reuse and change safely.
**Violation looks like:** Functions that validate, compute, persist and format all at once.
**Typical severity:** medium

### CODE-G5 — Small functions at one level of abstraction
**Rule:** Keep functions short enough to read at a glance, without mixing high-level steps and low-level detail.
**Why:** Mixed levels hide the overall flow.
**Violation looks like:** A long function where orchestration and byte-level details interleave; deep nesting (more than 3 levels).
**Typical severity:** low

### CODE-G6 — DRY, but not prematurely
**Rule:** Remove real duplication of knowledge; don't abstract on the first repetition. The rule of three: extract when a pattern appears the third time or the copies must change together.
**Why:** Duplication drifts; premature abstraction couples unrelated code.
**Violation looks like:** Copy-pasted logic that must stay in sync; a generic helper with one caller and many flags.
**Typical severity:** medium

### CODE-G7 — Comments explain why
**Rule:** Comments give reasons, constraints and non-obvious context; the code says what.
**Why:** "What" comments go stale and repeat the code; "why" comments preserve decisions.
**Violation looks like:** Comments narrating each line; commented-out code; a tricky workaround with no explanation.
**Typical severity:** nit (low for an unexplained workaround)

### CODE-G8 — Remove dead code
**Rule:** Delete unused functions, parameters, branches and flags; version control remembers them.
**Why:** Dead code misleads readers and attracts changes for nothing.
**Violation looks like:** Unused parameters kept "for later"; `#[allow(dead_code)]` without a reason; unreachable branches.
**Typical severity:** low

### CODE-G9 — Fail fast, with clear messages
**Rule:** Detect invalid input or states as early as possible, and say what was wrong and with which value.
**Why:** Early, specific errors are cheap to fix; late, vague ones are expensive to debug.
**Violation looks like:** Errors like "invalid input" with no detail; silently defaulting on bad input; swallowing errors.
**Typical severity:** medium

### CODE-G10 — Least surprise
**Rule:** Functions do what their name and signature suggest, with no hidden side effects, and similar things behave similarly.
**Why:** Surprises cause misuse.
**Violation looks like:** A `get_` that writes; a function that mutates its argument unexpectedly; inconsistent parameter order across similar functions.
**Typical severity:** medium

### CODE-G11 — Prefer immutability
**Rule:** Make values immutable by default; limit the scope and lifetime of mutable variables.
**Why:** Fewer moving parts, easier reasoning, safer concurrency.
**Violation looks like:** Long-lived `mut` variables changed in many places; mutation used where a new value would be clearer.
**Typical severity:** nit

### CODE-G12 — Low coupling, high cohesion
**Rule:** Depend on the minimum you need (narrow parameters and interfaces), and keep related logic together.
**Why:** Changes stay local.
**Violation looks like:** Functions taking a large context struct to read one field; feature logic split across distant files for no reason.
**Typical severity:** low
