import { createMemoryHistory, createRootRoute, createRouter, RouterProvider } from "@tanstack/react-router";
import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { ComingUp } from "./ComingUp";
import { summary } from "./fixtures";

it("pinned_items_link_to_question_pages", async () => {
  const rootRoute = createRootRoute({ component: () => <ComingUp summary={summary} /> });
  const router = createRouter({
    routeTree: rootRoute,
    history: createMemoryHistory({ initialEntries: ["/"] }),
  });
  render(<RouterProvider router={router} />);
  const link = await screen.findByRole("link", { name: /Why are AC voltages quoted as RMS/ });
  expect(link).toHaveAttribute("href", "/questions/2");
  expect(screen.getByText(/22 new questions waiting/)).toBeInTheDocument();
  expect(screen.getByText(/about 8 days/)).toBeInTheDocument();
});
