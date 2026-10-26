---
name: review-code
description: Review committed code in this repo for general coding best practices — naming, constants and magic values, function design, duplication, comments, error messages, dead code, coupling, test-driven development and test quality — against general principles and this repo's conventions, grading each finding high/medium/low/nit. Use after building any new work, alongside the backend/database/frontend reviews.
---

# General code review

**Scope:** every changed source file (Rust, TypeScript, SQL, shell, skill Markdown where it is
code-like). This skill covers language-agnostic quality; layering and layer-specific rules belong to
`/review-backend`, `/review-database` and `/review-frontend`.

1. Read and follow [`../_review/procedure.md`](../_review/procedure.md).
2. Check the diff against every entry in [`principles.md`](principles.md) (CODE-G) and
   [`conventions.md`](conventions.md) (CODE-R).
3. Report in the procedure's format under the heading `review-code`.
