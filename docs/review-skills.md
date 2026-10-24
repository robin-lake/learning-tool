# Architecture review skills

Four project-scoped Claude Code skills check committed work for structural problems: boundaries,
layering, consistency and design. They complement `/code-review`, which hunts for bugs.

| Skill | Reviews | Lists |
|---|---|---|
| `/review-backend` | Rust crates: layering, boundaries, errors, async, APIs | [principles](../.claude/skills/review-backend/principles.md) · [conventions](../.claude/skills/review-backend/conventions.md) |
| `/review-database` | Migrations, schema, SQL in code | [principles](../.claude/skills/review-database/principles.md) · [conventions](../.claude/skills/review-database/conventions.md) |
| `/review-frontend` | The React app (milestone 3+) | [principles](../.claude/skills/review-frontend/principles.md) · [conventions](../.claude/skills/review-frontend/conventions.md) |
| `/review-code` | Language-agnostic: naming, constants, function design, duplication, comments | [principles](../.claude/skills/review-code/principles.md) · [conventions](../.claude/skills/review-code/conventions.md) |

The shared steps (what range to review, how to grade, the report format) are in
[`.claude/skills/_review/procedure.md`](../.claude/skills/_review/procedure.md).

## When to run them

The main use: **after building new work**, the agent runs the relevant reviews on its diff, fixes the
high findings (and normally the medium ones in code it just wrote), and reports what it left.

By default a review covers:
- everything since `main` on a branch,
- unpushed commits on `main`,
- otherwise the last commit.

Pass a range to review something else, e.g. `/review-database HEAD~3..HEAD`.

Severity levels: **high (blocking)** · **medium** · **low** · **nit**. The table in the procedure says
what each means and how to triage it.

## Growing the lists

Each skill has two lists:

- **`principles.md`**: general best practices that apply to any project. Add to these as you learn.
- **`conventions.md`**: how *this* repo does things. Add one when a real decision settles a pattern;
  each entry records where or why it was decided. Reviews propose candidates under
  *Convention candidates*.

Entry format:

```
### BE-G3 — Keep I/O at the edges
**Rule:** one line.
**Why:** why it matters.
**Violation looks like:** what to spot in a diff.
**Typical severity:** high | medium | low | nit
**Example:** optional.
**Decided:** where/why (conventions only).
```

- **IDs** are `<BE|DB|FE|CODE>-<G|R><n>`. Take the next number in the file and never reuse one.
- **Retiring** an entry: mark its heading *retired* and leave the text, so old reports still resolve.

## Experimental: principles as study material

*Uncertain. Delete this section if it doesn't prove useful; nothing else depends on it.*

The general `principles.md` files could double as references for a "Software architecture" topic in the
learning tool. The loop would work like this:
- `/build-topic` reads them as sources.
- Principles that keep coming up in review findings are good candidates for questions.
- Principles learned through study come back as new entries.
