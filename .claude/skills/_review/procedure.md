# Architecture review procedure

Shared by `/review-backend`, `/review-database`, `/review-frontend` and `/review-code`. Each skill
supplies its **scope** (which files it reviews) and two lists:

- `principles.md`: general best practices (IDs `<AREA>-G<n>`)
- `conventions.md`: this repo's conventions (IDs `<AREA>-R<n>`)

These reviews are about **structure**: boundaries, layering, consistency and design. Bug hunting is the
job of `/code-review`. Report only from your own skill's two lists; another skill owns its own concerns.

## 1. Pick what to review

Use the first that applies:

1. A commit, range or branch passed as an argument (e.g. `HEAD~3..HEAD`, `abc123`).
2. On a branch other than `main`: `git diff main...HEAD`.
3. On `main` with unpushed commits: `git diff origin/main..HEAD`.
4. Otherwise: the last commit, `git show HEAD`.

Include uncommitted changes only if asked. Get the file list first (`git diff --name-only <range>`),
then the diff.

Always state the range at the top of the report, and say which rule picked it (e.g. "on main, nothing
unpushed → reviewing the last commit").

## 2. Filter to scope

Keep only the changed files inside the skill's scope. If none are, report
`<skill>: nothing in scope for <range>.` and stop. Never review out-of-scope files instead.

## 3. Read with context

Read the diff, and then enough of the surrounding code (the whole changed function, its callers, the
module it lives in) to judge structure. A line can look fine in a diff and still cross a boundary.

Read the **tests** in the diff as well: they say what the change is meant to do. This repo works
test-first ([`docs/research/testing.md`](../../../docs/research/testing.md)). Each skill has entries for
its own tests, and `/review-code` owns the general TDD checks (CODE-G13, CODE-G14, CODE-R7).

## 4. Check every entry

Go through **every** entry in both lists (skip entries marked *retired*). For each one, ask whether the
diff introduces or worsens a violation.

- **Introduced or worsened by the diff** → a finding.
- **Pre-existing, merely nearby** → at most a one-line **Noticed** note. Don't blame the diff for it.
- **Unsure** → say so in the finding rather than presenting a guess as fact.
- **Owned more specifically by another review skill** (e.g. a missing test for new SQL is DB-R9, not
  BE-G10) → leave it to that skill; don't report the same line twice across reviews.

## 5. Grade severity

Judge each finding by what it costs if it ships, not by how the entry is worded. The entry's
**Typical severity** is a starting point; grade above or below it when the context warrants, and say
why in a few words.

| Severity | Meaning | Triage |
|---|---|---|
| **High (blocking)** | Breaks an invariant or boundary: risk of data loss or corruption, wrong behavior, a security problem, or a structural choice that would be expensive to undo once other code builds on it. | Fix before the work lands. |
| **Medium** | A real structural cost: harder to change, test or understand, or it spreads a pattern the codebase avoids. | Fix in new code; existing instances can wait. |
| **Low** | A minor deviation with little lasting cost. | Fix if cheap; fine to leave. |
| **Nit** | Style or taste; no structural effect. | Optional. |

## 6. Report

```
## <skill> — <range> (<how the range was chosen>)
<N> high, <N> medium, <N> low, <N> nit

### High
- **<ID> <entry name>** — `path/to/file.rs:42`
  <What is wrong and why it matters, 1–2 sentences.> <Graded above typical because …, if so.>
  Fix: <concrete suggestion>

### Medium
…

### Low
…

### Nit
…

### Noticed (pre-existing)
- `file:line` — <one line> (<ID>)

### Convention candidates
- <A pattern this diff repeats or settles that isn't written down yet, phrased as a proposed
  `<AREA>-R<next>` entry with a one-line rule and why.>
```

Leave out empty sections. If there are no findings, say so plainly: `No findings.`, still followed by any
convention candidates.

## 7. Editing policy

The review **reports; it does not edit code**, unless asked.

Exception: when the agent reviews its **own** work right after building it, it may fix findings it
agrees with, each fix in its own commit:

- always the **high** findings,
- normally the **medium** findings in code it just wrote,
- low and nit at its discretion.

Afterwards it lists which findings it left unfixed, and why.

Never add, change or retire entries in `principles.md` or `conventions.md` on your own; propose
them under **Convention candidates** and let Robin decide.
