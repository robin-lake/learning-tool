import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { MapLegend } from "./MapLegend";

const stale = { after: 14, full: 60 };

it("legend_changes_with_color_mode", () => {
  const { rerender } = render(<MapLegend mode="combined" stale={stale} />);
  expect(screen.getByText("Last score")).toBeInTheDocument();
  expect(screen.getByText("Pale = fragile, deep = durable")).toBeInTheDocument();
  expect(screen.getByText("Faded = not studied in 14+ days")).toBeInTheDocument();
  expect(screen.getByText("Not studied")).toBeInTheDocument();

  rerender(<MapLegend mode="recall" stale={stale} />);
  expect(screen.getByText("Recall now")).toBeInTheDocument();
  expect(screen.queryByText("Pale = fragile, deep = durable")).toBeNull();

  rerender(<MapLegend mode="staleness" stale={stale} />);
  expect(screen.getByText("Last studied")).toBeInTheDocument();
  expect(screen.getByText("60+ days ago")).toBeInTheDocument();
  expect(screen.getByText("within 14 days")).toBeInTheDocument();
});
