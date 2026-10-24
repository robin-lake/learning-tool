# Frontend conventions (this repo)

The web app starts in milestone 3, so these entries record the **chosen stack** and will be expanded once
code exists. Each entry says where it was decided. Never reuse an ID.

### FE-R1 — React + TypeScript, strict
**Rule:** The frontend is React with TypeScript in strict mode; no plain `.js` source files.
**Why:** Chosen for the mature ecosystem; strict typing pairs with generated API types.
**Violation looks like:** `.js`/`.jsx` source files; `strict` off; `// @ts-ignore` without a reason.
**Typical severity:** medium
**Decided:** CLAUDE.md (Frontend). *To expand once code exists.*

### FE-R2 — TanStack Router owns routes and the map's zoom path
**Rule:** Routes are defined with TanStack Router; the map's drill-down path and the history-playback date live in typed route/search params.
**Why:** Breadcrumbs, back/forward and shareable links fall out of the URL.
**Violation looks like:** Ad-hoc routing; zoom path or selected day only in component state.
**Typical severity:** medium
**Decided:** Dependencies (chosen). *To expand once code exists.*

### FE-R3 — TanStack Query for server data
**Rule:** All API data is read and mutated through TanStack Query hooks, with one query-key scheme.
**Why:** One cache, consistent loading and error states, correct invalidation after mutations.
**Violation looks like:** Manual `fetch` in components; duplicate keys for the same resource.
**Typical severity:** medium
**Decided:** Dependencies (chosen). *To expand once code exists.*

### FE-R4 — API types are generated from Rust
**Rule:** Types for API data come from `ts-rs` output generated from the Rust structs; never hand-write them.
**Why:** One definition of each shape across backend and frontend.
**Violation looks like:** A hand-written interface mirroring a Rust struct; edits to generated files.
**Typical severity:** high
**Decided:** CLAUDE.md (Frontend).

### FE-R5 — shadcn/Radix components and design tokens
**Rule:** UI primitives come from shadcn/ui (Radix); colors and spacing use the theme tokens, with light and dark modes.
**Why:** Accessible primitives and a consistent look without one-off styling.
**Violation looks like:** Hand-rolled dialogs, menus or tooltips; hard-coded hex colors in components.
**Typical severity:** low
**Decided:** Dependencies (chosen). *To expand once code exists.*

### FE-R6 — d3 computes, React renders
**Rule:** Use `d3-hierarchy` and `d3-scale` for layout math and color scales only; React renders the SVG. No d3 DOM manipulation.
**Why:** One rendering model, with animation through `motion`.
**Violation looks like:** `d3.select(...).append(...)`; mixing d3 and React ownership of the same nodes.
**Typical severity:** medium
**Decided:** Dependencies (chosen).

### FE-R7 — The backend owns knowledge state
**Rule:** Strength, recency, size and "as of date" knowledge state come from the backend; the frontend renders them and doesn't recompute scheduling.
**Why:** Keeps the 2D map, the future 3D view and the study sessions consistent.
**Violation looks like:** FSRS or retrievability math in TypeScript; client-side reconstruction of history.
**Typical severity:** high
**Decided:** CLAUDE.md (Design principles: keep rendering separate from data).
