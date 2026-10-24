---
name: review-database
description: Review committed database changes in this repo — SQLite migrations, schema, and SQL in Rust code — for architectural problems such as missing constraints, unsafe migrations, transaction misuse, history loss and indexing, against general principles and this repo's conventions, grading each finding high/medium/low/nit. Use after building work that touches the schema or queries.
---

# Database architecture review

**Scope:** `crates/core/migrations/*.sql`, plus SQL strings and transaction usage in `crates/**/*.rs`.
Rust structure around the queries belongs to `/review-backend`.

1. Read and follow [`../_review/procedure.md`](../_review/procedure.md).
2. Check the diff against every entry in [`principles.md`](principles.md) (DB-G) and
   [`conventions.md`](conventions.md) (DB-R).
3. Report in the procedure's format under the heading `review-database`.
