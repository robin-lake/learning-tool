# Database principles (general)

Applies to any relational database. Add entries as you learn; never reuse an ID.

### DB-G1 — Enforce invariants in the schema
**Rule:** Express rules the data must always satisfy as constraints: `NOT NULL`, `CHECK`, `FOREIGN KEY`, `UNIQUE`.
**Why:** The database is the last line of defense; constraints hold even when application code has a bug.
**Violation looks like:** New columns that are nullable without reason; enum-like text columns without `CHECK`; references without a foreign key; uniqueness enforced only in code.
**Typical severity:** medium

### DB-G2 — Migrations are forward-only and immutable
**Rule:** Never edit a migration that has been applied anywhere; change the schema with a new migration.
**Why:** Databases record which migrations ran; an edited migration silently diverges or fails checksum checks.
**Violation looks like:** A diff that modifies an existing `migrations/*.sql` file instead of adding one.
**Typical severity:** high

### DB-G3 — Keep transactions short and complete
**Rule:** A transaction wraps exactly the reads and writes that must be atomic, and nothing slow (network, large file I/O, user waits) runs inside it.
**Why:** Long transactions hold locks and block other writers; incomplete ones allow partial writes.
**Violation looks like:** Dependent writes outside a transaction; file copies or HTTP calls while a write transaction is open.
**Typical severity:** medium (high if dependent writes aren't atomic)

### DB-G4 — Index for the queries you run
**Rule:** Add indexes that match real `WHERE`/`JOIN`/`ORDER BY` patterns on tables that grow; don't add speculative ones.
**Why:** Missing indexes make queries slow as data grows; unused indexes slow writes and waste space.
**Violation looks like:** A new query filtering a growing table on an unindexed column; an index nothing queries.
**Typical severity:** low (medium on hot paths)

### DB-G5 — One source of truth; derived data is rebuildable
**Rule:** Store each fact once. Denormalized or cached data must be derivable from the source and rebuildable.
**Why:** Duplicated facts drift apart; rebuildable caches can be repaired.
**Violation looks like:** The same value maintained in two tables; a summary column with no way to recompute it.
**Typical severity:** medium

### DB-G6 — Parameterize every query
**Rule:** Values go in bound parameters, never spliced into SQL text.
**Why:** Prevents injection and quoting bugs, and lets the database reuse plans.
**Violation looks like:** `format!` or string concatenation building SQL with values.
**Typical severity:** high

### DB-G7 — Explicit columns on writes
**Rule:** `INSERT` and `UPDATE` name their columns.
**Why:** Adding a column later can't silently shift values into the wrong place.
**Violation looks like:** `INSERT INTO t VALUES (...)` without a column list.
**Typical severity:** low

### DB-G8 — Store time unambiguously
**Rule:** Store instants in UTC with an explicit format; store calendar dates separately from instants when the domain has "days".
**Why:** Local times are ambiguous across zones and DST; mixing dates and instants causes off-by-one-day bugs.
**Violation looks like:** Local timestamps in storage; a date derived by truncating a UTC instant.
**Typical severity:** medium

### DB-G9 — Preserve history users care about
**Rule:** Prefer archiving or status changes over hard deletes for records that carry history.
**Why:** Deleted history can't be recovered, and other records may depend on it.
**Violation looks like:** `DELETE` on user-meaningful rows; `ON DELETE CASCADE` that would wipe history.
**Typical severity:** high

### DB-G10 — Schema changes are safe on existing data
**Rule:** A migration must work on a populated database: give new `NOT NULL` columns defaults or a backfill, and consider existing rows when adding constraints.
**Why:** Migrations that only work on empty databases break real installs.
**Violation looks like:** `ADD COLUMN ... NOT NULL` without a default; a new `UNIQUE`/`CHECK` that existing rows may violate.
**Typical severity:** high

### DB-G11 — Test against a real database built by the real migrations
**Rule:** Database code is tested against a real database, never a mock, built by running the actual migrations, with one fresh database per test and production pragmas (foreign keys on). Schema rules are test-driven too: a new constraint gets a test that a violating write is rejected.
**Why:** SQL only proves itself when it runs; tests on the migrated schema also prove a fresh install works, and isolated databases keep tests order-independent.
**Violation looks like:** Hand-written test DDL instead of migrations; a mocked repository standing in for SQL; tests sharing rows; a new `CHECK`, `UNIQUE` or foreign key with no test that it rejects bad data.
**Typical severity:** medium
