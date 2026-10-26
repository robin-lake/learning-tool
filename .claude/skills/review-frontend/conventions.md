# Frontend conventions (this repo)

Recorded from the chosen stack and the milestone 3 build. Each entry says where it was decided. Never reuse an ID.

### FE-R1 — React + TypeScript, strict
**Rule:** The frontend is React with TypeScript in strict mode; no plain `.js` source files.
**Why:** Chosen for the mature ecosystem; strict typing pairs with generated API types.
**Violation looks like:** `.js`/`.jsx` source files; `strict` off; `// @ts-ignore` without a reason.
**Typical severity:** medium
**Decided:** CLAUDE.md (Frontend); milestone 3.

### FE-R2 — TanStack Router owns routes and the map's zoom path
**Rule:** Routes are file-based TanStack Router routes in `web/src/routes/`. Navigable state lives in typed route/search params (`validateSearch`), with optional params present only when set (e.g. topic `all`/`archived`). The map's drill-down path and the history-playback date will follow the same pattern.
**Why:** Breadcrumbs, back/forward and shareable links fall out of the URL.
**Violation looks like:** Ad-hoc routing; zoom path or selected day only in component state.
**Typical severity:** medium
**Decided:** Dependencies (chosen); milestone 3.

### FE-R3 — TanStack Query for server data
**Rule:** All API data goes through `web/src/api/queries.ts`: query options (`homeQuery`, `topicQuery`, …) with the `keys` scheme, and mutation hooks that refresh affected queries. Components never call `fetch`; the `api()` client in `client.ts` is the only caller and turns errors into `ApiError` (`kind`, `status`). Failed queries render `ErrorState` with Retry.
**Why:** One cache, consistent loading and error states, correct invalidation after mutations.
**Violation looks like:** Manual `fetch` in components; duplicate keys for the same resource.
**Typical severity:** medium
**Decided:** Dependencies (chosen); milestone 3.

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
**Decided:** Dependencies (chosen); milestone 3.

### FE-R6 — d3 computes, React renders
**Rule:** Use `d3-hierarchy` for layout math only; React renders. No d3 DOM manipulation. Pure layout and color logic lives in `web/src/lib/` with unit tests (like `lib/heatmap.ts` and `lib/map/`). Map layouts implement the `Layout` type in `lib/map/layout.ts`, so circle packing can be swapped for a treemap without touching the components.
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

### FE-R8 — Ignore macOS sidecar files everywhere
**Rule:** Every tool that globs source files (tsconfig, Biome, Vitest, the router plugin) excludes `._*`; `web/.npmrc` keeps `node-linker=hoisted` and `package-import-method=copy`.
**Why:** The repo can live on exFAT, where macOS writes `._*` files beside every file and symlinks don't exist; a stray `._index.tsx` would otherwise become a route or a compile error.
**Violation looks like:** A new config or glob that would pick up `._*` files; switching pnpm back to symlinked installs.
**Typical severity:** medium
**Decided:** Milestone 3 (Task 7).

### FE-R9 — Text follows the typography guide
**Rule:** Reading text, headings and labels follow [`docs/research/typography.md`](../../../docs/research/typography.md) (from Butterick's *Practical Typography*): 16 px body text with relaxed line spacing, reading columns of 45–90 characters, sentence-case headings, sparing emphasis, and all caps only for short labels with at least `tracking-wider`.
**Why:** Most of the app is reading answers and feedback; comfortable text is what makes reviewing pleasant enough to come back to.
**Violation looks like:** Answers or feedback in `text-xs`/`text-sm`; paragraphs spanning the full page width; tiny all-caps section labels; `uppercase` with `tracking-wide` or less; bold and italic together.
**Typical severity:** low
**Decided:** Question page restyle after milestone 4.

### FE-R10 — Frontend tests use this repo's seams
**Rule:** Pure logic in `web/src/lib/` has Vitest unit tests. Components render through `renderWithApp` (fresh `QueryClient`, retries off) with the network mocked only via `mockFetch`. Pages are tested by rendering the real `routeTree` in a memory history and asserting on `router.state.location`. Time uses fake timers or stubbed `requestAnimationFrame`/`performance.now`. See [`docs/research/testing.md`](../../../docs/research/testing.md).
**Why:** One network seam and an isolated cache per test keep tests fast and independent (TkDodo, *Testing React Query*); the real route tree tests URL state end to end.
**Violation looks like:** Mocking hooks or `api()` directly; a shared `QueryClient` across tests; real `setTimeout` waits; a new route or lib module with no test.
**Typical severity:** medium
**Decided:** Milestones 3–4 (how the suites grew); TDD made explicit 2026-10-26.
