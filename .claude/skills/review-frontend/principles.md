# Frontend principles (general)

Applies to any React/TypeScript frontend. Add entries as you learn; never reuse an ID.

### FE-G1 — Server state is not UI state
**Rule:** Data owned by the server is fetched and cached by a server-state library; component state holds only UI concerns (open/closed, input drafts, selection).
**Why:** Copying server data into local state creates stale duplicates and hand-rolled caching.
**Violation looks like:** `useEffect` + `fetch` + `useState` to load data; server data copied into a store and edited there.
**Typical severity:** medium

### FE-G2 — Minimal, colocated state
**Rule:** Keep state as close as possible to where it's used, and store only what can't be derived.
**Why:** Less state means fewer ways to get out of sync and fewer re-renders.
**Violation looks like:** Global state for one component's concern; storing values computable from props or other state; `useEffect` that syncs one state variable from another.
**Typical severity:** medium

### FE-G3 — Components have one responsibility
**Rule:** Split components by responsibility (data, layout, interaction, presentation); a component that is hard to name does too much.
**Why:** Focused components are easier to reuse, test and change.
**Violation looks like:** A several-hundred-line component mixing fetching, transforms, layout and event handling.
**Typical severity:** low (medium when it blocks reuse)

### FE-G4 — Render is pure
**Rule:** Rendering computes UI from props and state with no side effects; effects run only to synchronize with external systems.
**Why:** React may render more than once; side effects in render cause duplicate requests and bugs.
**Violation looks like:** Mutations, requests or logging during render; `useEffect` used for derived values or event responses.
**Typical severity:** medium

### FE-G5 — The URL holds navigable state
**Rule:** State a user would bookmark, share or reach with back/forward (current page, selected item, zoom path, filters) lives in the URL.
**Why:** Back/forward and reload work as people expect.
**Violation looks like:** Navigation or drill-down state held only in component state.
**Typical severity:** medium

### FE-G6 — A typed API boundary
**Rule:** Data from the network is typed at the boundary from a single source, and components consume those types.
**Why:** Shape mismatches surface at compile time instead of in the UI.
**Violation looks like:** `any` or untyped JSON flowing into components; the same response type declared in several places.
**Typical severity:** medium

### FE-G7 — Accessible by default
**Rule:** Use semantic elements and accessible primitives: buttons for actions, labels for inputs, keyboard support, sufficient contrast, and text alternatives for visual-only information.
**Why:** Usable by keyboard and assistive tech, and usually better structured.
**Violation looks like:** Clickable `div`s; color as the only signal (e.g. strength shown only by hue); tooltips reachable only by hover.
**Typical severity:** medium

### FE-G8 — Every async view has loading, error and empty states
**Rule:** Anything that loads data renders a designed state for loading, failure and no data.
**Why:** Blank screens and silent failures erode trust.
**Violation looks like:** A view that renders nothing while loading or on error; a list with no empty message.
**Typical severity:** low

### FE-G9 — Avoid deep prop drilling
**Rule:** When the same props pass through more than about two intermediate components that don't use them, restructure (composition, context, or moving state).
**Why:** Drilling couples unrelated components and makes changes ripple.
**Violation looks like:** Props threaded through layers that only forward them.
**Typical severity:** low

### FE-G10 — Expensive visuals are isolated
**Rule:** Heavy rendering (large SVGs, canvases, animations) lives in its own components with stable inputs, so unrelated state changes don't re-render it.
**Why:** Keeps interactions smooth as data grows.
**Violation looks like:** A big chart re-rendering on every keystroke elsewhere; layout math recomputed on every render.
**Typical severity:** low (medium for the map and playback)

### FE-G11 — Test the way people use the app
**Rule:** Component tests find elements as a user would (`getByRole` with a name first, then label or text, test IDs last), wait with `findBy*`, assert with jest-dom matchers on what the user sees, and use `queryBy*` only to assert absence. Pure logic gets plain unit tests. Write the test first.
**Why:** Testing Library's guiding principle: tests that resemble real use give the most confidence and survive refactors; a role query that fails often reveals an accessibility gap.
**Violation looks like:** `container.querySelector` or `getByTestId` where a role works; `waitFor(() => getBy…)`; assertions on component state, props or hook calls; raw `.disabled` checks instead of `toBeDisabled()`.
**Typical severity:** low
