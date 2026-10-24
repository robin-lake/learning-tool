# Future feature: model-agnostic study clients

Status: idea (not scheduled). Written 2026-10-24.

## Goal

Studying, building topics and refining questions currently run in Claude Code. The goal is to make the same workflows usable from other agent tools. The first target is [OpenCode](https://opencode.ai), which is model-agnostic, so any capable LLM provider could quiz Robin without a second copy of the logic to maintain.

## What's already portable

The architecture keeps the provider-specific part small:

- **All data and rules live behind the MCP server** (`learning-tool mcp`): scheduling, grading arithmetic, pins, references, the guide. MCP is an open protocol, and OpenCode runs local MCP servers from its config ([OpenCode MCP docs](https://opencode.ai/docs/mcp-servers)).
- **The workflows are `SKILL.md` files.** OpenCode reads skills in the same format, including from `~/.claude/skills/` and `.claude/skills/` ([OpenCode skills docs](https://opencode.ai/docs/skills)). It only recognizes `name` and `description` in the frontmatter, which is all ours use.
- **The model's job is limited to asking, judging rubric points, and writing feedback.** Grades, intervals and every number are computed in Rust, so a different model can't drift the scheduler.

## What ties it to Claude Code today

1. **Tool names in the skills** use Claude Code's naming, `mcp__learning-tool__start_session`. Other clients name MCP tools differently.
2. **Claude-specific wording and tools:**
   - "Claude", "Claude Code" and "/study in Claude Code" in skill text, in the web app's hints, and in the MCP tool descriptions.
   - `/build-topic` assumes web search and fetch tools (WebSearch/WebFetch).
3. **Install:** `scripts/install.sh` registers the server with `claude mcp add` only.
4. **Grading quality depends on the model.** Rubric judgment is the one place a weaker model would hurt: lenient grading inflates intervals, and harsh grading creates false leeches. Nothing records which model graded a review.

## Approaches

- **A. One portable skill set (recommended).**
  - Keep a single `skills/` folder and make it client-neutral:
    - refer to tools by their plain names ("call `start_session` from the learning-tool server")
    - say "the assistant" rather than "Claude"
    - describe research steps in terms of capabilities ("search the web and fetch pages") rather than tool names
  - OpenCode already discovers these files, so there's only one copy to maintain.
  - Risk: some models follow loosely worded skills less reliably. Mitigate with the evals below.
- **B. A shared source with per-client builds.**
  - Write each workflow once with placeholders (`{{tool:start_session}}`, `{{assistant}}`), and generate `skills/claude/` and `skills/opencode/` with a small script.
  - Choose this only if A turns out too vague for one client. It adds a build step and two outputs to keep in sync.
- **C. Move the workflows into MCP prompts.**
  - MCP servers can expose *prompts*, named templates that clients may offer as slash commands. The server would then carry the workflows itself, and every MCP client would get them for free.
  - Rejected for now: it goes against the convention that Rust holds no prompt text (BE-R7), editing a workflow would mean rebuilding the binary, and client support for prompts varies. Worth revisiting if prompt support becomes universal.

## Sketch of the work (approach A)

1. **Neutral skills.** Rewrite tool references and assistant wording in `skills/*/SKILL.md` and in the MCP tool descriptions. Add a short "tools you'll need" list at the top of each skill.
2. **Record the grader.**
   - Add a nullable `grader` column to `reviews`, holding the client and model that judged the answer (for example `claude-code/claude-opus-5-5`, `opencode/<provider>/<model>`).
   - `record_review` gains an optional `grader` field. The skills ask the assistant to fill it in if it knows its own model.
   - The question page shows it in the attempt history.
   - This lets Robin compare grading across models, and later weight or re-check reviews from a model that turns out unreliable. The review log stays append-only.
3. **Install for both clients.**
   - `install.sh` detects `claude` and/or `opencode`.
   - For OpenCode, it adds a `"learning-tool": {"type": "local", "command": ["<abs path>/learning-tool", "mcp"]}` entry to `~/.config/opencode/opencode.json`. Check the schema version first: the docs show v1 (`mcp`) and v2 (`mcp.servers`) layouts.
   - It relies on OpenCode's existing `~/.claude/skills` discovery, or links into `~/.config/opencode/skills/`.
4. **Client-neutral hints in the web app.** For example, "run /study in your assistant".
5. **Grading evals.** A small fixed set of real questions with Robin's answers and the expected rubric hits (taken from the review log), plus a script that has a given model grade them and reports agreement. This tells Robin whether a model is good enough to grade before trusting it with the schedule.

## Open questions

- Which providers and models are realistic targets? Local models may be too weak for L2/L3 rubric judgment.
- Should a session be allowed to mix graders, or should a "trusted grader" setting gate which reviews update the schedule?
- Does OpenCode expose skills as slash commands (`/study`), or does Robin invoke them by asking? This affects the "zero friction to start" principle.
