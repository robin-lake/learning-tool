import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { timeline } from "@/lib/map/fixtures";
import { routeTree } from "@/routeTree.gen";
import { mockFetch } from "@/test/render";

afterEach(() => vi.unstubAllGlobals());

function open(url: string) {
  vi.stubGlobal("fetch", mockFetch(200, timeline).fn);
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: [url] }) });
  render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

it("opens_top_level_circles", async () => {
  open("/map");
  expect(await screen.findByRole("button", { name: /^Electric circuits: 2 questions/ })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^Speaker design tools/ })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /^q1/ })).toBeNull();
  expect(screen.getByRole("link", { name: "Map" })).toHaveAttribute("aria-current", "page");
});

it("zoom_updates_search_and_breadcrumbs", async () => {
  const router = open("/map");
  fireEvent.click(await screen.findByRole("button", { name: /^Electric circuits/ }));
  expect(await screen.findByRole("button", { name: /^q1/ })).toBeInTheDocument();
  expect(router.state.location.search).toEqual({ topic: "t1" });
  const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
  expect(crumbs).toHaveTextContent("Topics › Electric circuits");
});

it("unknown_zoom_topic_falls_back", async () => {
  open("/map?topic=t99");
  expect(await screen.findByText(/That topic isn't on the map/)).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /^Electric circuits/ })).toBeInTheDocument();
});

it("zoomed_empty_day_shows_message", async () => {
  open("/map?topic=t2&day=2026-10-01");
  expect(await screen.findByText("Nothing here yet on 1 Oct 2026")).toBeInTheDocument();
});

it("color_by_updates_search", async () => {
  const router = open("/map");
  fireEvent.change(await screen.findByRole("combobox", { name: "Color by" }), {
    target: { value: "recall" },
  });
  expect(router.state.location.search).toEqual({ color: "recall" });
  expect(screen.getByText("Recall now")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Fixed" }));
  expect(router.state.location.search).toEqual({ color: "recall", mode: "fixed" });
});

it("map_fits_its_container_after_loading", async () => {
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(480);
  open("/map");
  await screen.findByRole("button", { name: /^Electric circuits/ });
  const svg = screen.getByTitle("Map of Topics").closest("svg");
  expect(svg).toHaveAttribute("width", "480");
  vi.restoreAllMocks();
});

it("escape_zooms_out", async () => {
  const router = open("/map?topic=t1");
  await screen.findByRole("button", { name: /^q1/ });
  fireEvent.keyDown(window, { key: "Escape" });
  expect(await screen.findByRole("button", { name: /^Electric circuits/ })).toBeInTheDocument();
  expect(router.state.location.search).toEqual({});
});

it("playback_writes_the_url_only_when_it_stops", async () => {
  vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance"] });
  const router = open("/map");
  fireEvent.click(await screen.findByRole("button", { name: "Play" }));
  act(() => vi.advanceTimersByTime(2600));
  const slider = screen.getByRole("slider", { name: "Day" }) as HTMLInputElement;
  expect(slider.value).toBe("1");
  expect(router.state.location.search).toEqual({});
  fireEvent.click(screen.getByRole("button", { name: "Pause" }));
  await vi.waitFor(() => expect(router.state.location.search).toEqual({ day: "2026-10-02" }));
  vi.useRealTimers();
});
