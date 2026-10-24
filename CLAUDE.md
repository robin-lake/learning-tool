# Learning Tool

A personal spaced-repetition learning tool (Anki-like). Claude asks the user questions, the user's answers are graded (again / hard / good / easy), and a scheduler decides when each question comes back. A local web app shows progress and a map of what the user knows.

**Status: planning.** No code yet. Design docs go in `docs/superpowers/specs/`, implementation plans in `docs/superpowers/plans/`. Update this file when decisions are made — don't let it drift from reality.

## Decided

- **Backend:** Rust.
- **Storage:** a local SQLite database. Single user, runs on the user's machine.
- **Frontend:** React + TypeScript (Vite, TanStack, shadcn). The goal is a tool to actually use, so pick the mature ecosystem. API types are generated from the Rust structs (e.g. `ts-rs` / `specta`) rather than hand-written. The 3D stretch goal may be a Rust/WASM module embedded in the React app.
- **Quizzing happens in Claude Code**, not in the web app. Claude reads due questions and records grades against the local database (likely via an MCP server sharing a core crate with the web backend). The web app is for tracking, visualization, and project features.
- **Projects page** (learning while building real projects) is a separate, later plan. Don't design it into v1 beyond leaving room for it.

## Open decisions

- **Claude ↔ database interface:** MCP server vs. CLI + skill; what tools it exposes.
- **Who writes questions and who grades:** Claude, the user, or both.
- **Scheduling algorithm:** FSRS (leaning) vs. SM-2.
- **Topic schema details** (see below).

## Product outline

- **Home:** navbar (Map, Topics, Projects); today summary (answered today, remaining today, current streak); GitHub-style activity heatmap whose day tooltips show question count, grade breakdown, and topics studied; remaining-questions section.
- **Map:** zoomable visual map of topics. Size = number of questions; color = answer strength (red weak → blue strong); saturation = recency (faded if not studied lately). Click to zoom into subtopics, breadcrumbs to go back, down to individual questions. A question opens its own page with question text and attempt history. Time controls step back/forward one day and a play button animates the whole history over a few seconds.
- **Topics:** a list of topics.
- **Projects:** later, separate plan.
- **Stretch:** GPU-accelerated 3D graph of topics and their cross-links, colored by strength, with tooltips and navigation between neighboring topics.

## Design principles

- **Topics nest and cross-link.** Topics form a hierarchy, and topics/questions can be linked to others with a strength ("how related"). Model links as weighted edges so the 2D map and a future 3D graph read the same data.
- **Review history is an append-only log.** Every grade is a stored event; anything shown for a past date (map playback, heatmap) is derived from the log, never from overwritten state.
- **Keep rendering separate from data.** The backend serves "knowledge state" (per topic/question strength, recency, size, links) as plain data, optionally as of a date. Views — 2D map, 3D graph — are clients of that.
