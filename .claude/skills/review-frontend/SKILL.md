---
name: review-frontend
description: Review committed frontend changes in this repo (the React + TypeScript web app) for architectural problems — state ownership, component boundaries, data fetching, routing, typed API boundary, accessibility — against general principles and this repo's conventions, grading each finding high/medium/low/nit. Use after building frontend work.
---

# Frontend architecture review

**Scope:** frontend source: `web/**` and any `*.ts`, `*.tsx`, `*.css` files. Until the web app exists
(milestone 3) this usually means "nothing in scope"; say so and stop. Naming and general code quality
belong to `/review-code`.

1. Read and follow [`../_review/procedure.md`](../_review/procedure.md).
2. Check the diff against every entry in [`principles.md`](principles.md) (FE-G) and
   [`conventions.md`](conventions.md) (FE-R).
3. Report in the procedure's format under the heading `review-frontend`.
