import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import type { TreeNode } from "@/lib/map/tree";
import { MapTooltip } from "./MapTooltip";

const node: TreeNode = {
  id: "t1",
  kind: "topic",
  refId: 1,
  label: "Electric circuits",
  createdOn: "2026-10-01",
  value: 17,
  due: 3,
  targetLevel: 2,
  measures: { studied: 12, recall: 0.741, durability: 9.4, lastScore: 0.8, daysSince: 4 },
  children: [],
};

it("shows_all_measures_and_dates", () => {
  render(<MapTooltip node={node} day="2026-10-24" />);
  expect(screen.getByText("Electric circuits")).toBeInTheDocument();
  expect(screen.getByText("17 questions, 12 studied")).toBeInTheDocument();
  expect(screen.getByText("Recall now: 74%")).toBeInTheDocument();
  expect(screen.getByText("Durability: 9 days")).toBeInTheDocument();
  expect(screen.getByText("Last score: 80%")).toBeInTheDocument();
  expect(screen.getByText("Last studied: 20 Oct 2026 (4 days ago)")).toBeInTheDocument();
  expect(screen.getByText("Due: 3")).toBeInTheDocument();
  expect(screen.getByText("Target: L2")).toBeInTheDocument();
});

it("never_studied_says_so", () => {
  render(
    <MapTooltip
      node={{
        ...node,
        measures: { studied: 0, recall: null, durability: null, lastScore: null, daysSince: null },
      }}
      day="2026-10-24"
    />,
  );
  expect(screen.getByText("Not studied yet")).toBeInTheDocument();
  expect(screen.queryByText(/Recall/)).toBeNull();
});
