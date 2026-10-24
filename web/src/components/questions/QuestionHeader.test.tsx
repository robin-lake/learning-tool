import { screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { renderWithApp } from "@/test/render";
import { page } from "./fixtures";
import { QuestionHeader } from "./QuestionHeader";

it("retired_banner_links_newest", async () => {
  const p = page({ newest_revision_id: 9 });
  p.detail.question.status = "retired";
  renderWithApp(<QuestionHeader page={p} />);
  const link = await screen.findByRole("link", { name: /newest version/i });
  expect(link).toHaveAttribute("href", "/questions/9");
  expect(screen.queryByRole("button", { name: "Edit" })).toBeNull();
});

it("active_question_shows_actions_and_level", async () => {
  renderWithApp(<QuestionHeader page={page({ pinned: true })} />);
  expect(await screen.findByRole("button", { name: /pinned/i })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Edit" })).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Suspend" })).toBeInTheDocument();
  expect(screen.getByText("L1")).toBeInTheDocument();
});
