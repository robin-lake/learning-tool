import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { ApiError } from "@/api/client";
import { ErrorState } from "./ErrorState";

it("query_error_shows_retry", async () => {
  const onRetry = vi.fn();
  render(<ErrorState error={new ApiError("internal", 500, "internal error")} onRetry={onRetry} />);
  expect(screen.getByText(/internal error/)).toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: /retry/i }));
  expect(onRetry).toHaveBeenCalledOnce();
});

it("network_error_mentions_the_server", () => {
  render(<ErrorState error={new ApiError("network", 0, "Failed to fetch")} onRetry={() => {}} />);
  expect(screen.getByText(/learning-tool serve/)).toBeInTheDocument();
});
