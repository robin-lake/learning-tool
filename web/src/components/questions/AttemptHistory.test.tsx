import { render, screen, within } from "@testing-library/react";
import { expect, it } from "vitest";
import { AttemptHistory } from "./AttemptHistory";
import { review } from "./fixtures";

it("rows_show_points_score_grade_newest_first", () => {
  render(
    <AttemptHistory
      currentId={2}
      reviews={[review(), review({ id: 2, reviewed_at: "2026-10-25T09:00:00Z", score: 1, grade: "easy" })]}
    />,
  );
  const rows = screen.getAllByRole("row").slice(1);
  expect(within(rows[0]).getByText("Easy")).toBeInTheDocument();
  expect(within(rows[0]).getByText("100%")).toBeInTheDocument();
  expect(within(rows[1]).getByText("Again")).toBeInTheDocument();
  expect(within(rows[1]).getByText("50%")).toBeInTheDocument();
  expect(within(rows[1]).getByText("✓ r1")).toBeInTheDocument();
  expect(within(rows[1]).getByText("✗ r2")).toBeInTheDocument();
  expect(screen.getAllByText(/RMS is 0.707 x peak, not half/).length).toBeGreaterThan(0);
});

it("earlier_wording_labeled", () => {
  render(<AttemptHistory currentId={5} reviews={[review({ question_id: 2 })]} />);
  expect(screen.getByText("earlier wording")).toBeInTheDocument();
});

it("empty_history_says_not_studied", () => {
  render(<AttemptHistory currentId={2} reviews={[]} />);
  expect(screen.getByText(/not answered yet/i)).toBeInTheDocument();
});
