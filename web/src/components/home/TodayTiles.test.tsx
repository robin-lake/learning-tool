import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { summary } from "./fixtures";
import { TodayTiles } from "./TodayTiles";

it("shows_counts_breakdown_and_best_streak", () => {
  render(<TodayTiles summary={summary} />);
  expect(screen.getByText("5")).toBeInTheDocument();
  expect(screen.getByText("Easy 1 · Good 0 · Hard 1 · Again 3")).toBeInTheDocument();
  expect(screen.getByText(/3 due tomorrow/)).toBeInTheDocument();
  expect(screen.getByText("1 day")).toBeInTheDocument();
  expect(screen.getByText("Best: 4 days")).toBeInTheDocument();
});
