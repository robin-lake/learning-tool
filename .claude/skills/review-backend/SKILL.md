---
name: review-backend
description: Review committed Rust backend changes in this repo for architectural problems — layering, crate and module boundaries, error handling, async, side effects, API shape, test-first tests at the right seams — against general principles and this repo's conventions, grading each finding high/medium/low/nit. Use after building backend work, or when asked to review backend structure.
---

# Backend architecture review

**Scope:** `crates/**/*.rs` and `Cargo.toml` files. SQL text, migrations and transaction usage belong
to `/review-database`; naming and general code quality belong to `/review-code`.

1. Read and follow [`../_review/procedure.md`](../_review/procedure.md).
2. Check the diff against every entry in [`principles.md`](principles.md) (BE-G) and
   [`conventions.md`](conventions.md) (BE-R).
3. Report in the procedure's format under the heading `review-backend`.
