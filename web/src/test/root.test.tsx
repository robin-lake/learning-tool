import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createMemoryHistory, createRouter, RouterProvider } from "@tanstack/react-router";
import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { routeTree } from "@/routeTree.gen";

it("unknown_route_shows_not_found", async () => {
  const router = createRouter({ routeTree, history: createMemoryHistory({ initialEntries: ["/nope"] }) });
  render(
    <QueryClientProvider client={new QueryClient()}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  expect(await screen.findByText(/page not found/i)).toBeInTheDocument();
  expect(screen.getByRole("link", { name: "Topics" })).toBeInTheDocument();
});
