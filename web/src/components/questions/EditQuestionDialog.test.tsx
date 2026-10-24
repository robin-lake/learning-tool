import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { mockFetch, renderWithApp } from "@/test/render";
import { EditQuestionDialog } from "./EditQuestionDialog";
import { page } from "./fixtures";

afterEach(() => vi.unstubAllGlobals());

it("editor_blocks_save_on_invalid_rubric", async () => {
  renderWithApp(<EditQuestionDialog question={page().detail.question} />);
  await userEvent.click(await screen.findByRole("button", { name: "Edit" }));
  const texts = screen.getAllByLabelText(/^Point \d text$/);
  await userEvent.clear(texts[0]);
  expect(screen.getByText("Point r1 has no text.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save new version" })).toBeDisabled();
});

it("save_navigates_to_new_revision", async () => {
  const f = mockFetch(200, { ...page().detail.question, id: 99, revision_of: 2 });
  vi.stubGlobal("fetch", f.fn);
  const { router } = renderWithApp(<EditQuestionDialog question={page().detail.question} />);
  await userEvent.click(await screen.findByRole("button", { name: "Edit" }));
  const prompt = screen.getByLabelText("Question");
  await userEvent.clear(prompt);
  await userEvent.type(prompt, "Why quote RMS?");
  await userEvent.click(screen.getByRole("button", { name: "+ Point" }));
  await userEvent.type(screen.getByLabelText("Point 3 text"), "Mentions power");
  await userEvent.click(screen.getByRole("button", { name: "Save new version" }));
  expect(f.calls.at(-1)).toMatchObject({ url: "/api/questions/2", method: "PATCH" });
  const sent = JSON.parse(f.calls.at(-1)?.body ?? "{}");
  expect(sent.prompt).toBe("Why quote RMS?");
  expect(sent.rubric.map((p: { id: string }) => p.id)).toEqual(["r1", "r2", "r3"]);
  await waitFor(() => expect(router.state.location.pathname).toBe("/questions/99"));
});
