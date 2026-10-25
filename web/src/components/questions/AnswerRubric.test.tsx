import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { AnswerRubric } from "./AnswerRubric";
import { page } from "./fixtures";

const q = page().detail.question;

it("answer_and_rubric_hidden_until_shown", () => {
  render(<AnswerRubric question={q} citation="Capacitor (part 1)" />);
  expect(screen.queryByText(q.answer)).toBeNull();
  expect(screen.queryByText("RMS = peak / sqrt(2)")).toBeNull();
  expect(screen.getByText("Capacitor (part 1)")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Show answer" }));
  expect(screen.getByText(q.answer)).toBeInTheDocument();
  expect(screen.getByText("RMS = peak / sqrt(2)")).toBeInTheDocument();
  expect(screen.getByText("Pass at 60%")).toBeInTheDocument();
  expect(screen.getByText("Essential")).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Hide answer" }));
  expect(screen.queryByText(q.answer)).toBeNull();
});

it("reveal_resets_for_another_question", () => {
  const { rerender } = render(<AnswerRubric question={q} citation={null} />);
  fireEvent.click(screen.getByRole("button", { name: "Show answer" }));
  rerender(<AnswerRubric question={{ ...q, id: 3, answer: "Another answer" }} citation={null} />);
  expect(screen.queryByText("Another answer")).toBeNull();
});
