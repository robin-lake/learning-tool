import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { mockFetch, renderWithApp } from "@/test/render";
import { row } from "./fixtures";
import { QuestionTable } from "./QuestionTable";

afterEach(() => vi.unstubAllGlobals());

it("pin_toggle_calls_pin_then_unpin", async () => {
  const f = mockFetch(200, {});
  vi.stubGlobal("fetch", f.fn);
  const { unmount } = renderWithApp(<QuestionTable rows={[row()]} />);
  const pin = await screen.findByRole("button", { name: /pin/i });
  expect(pin).toHaveAttribute("aria-pressed", "false");
  await userEvent.click(pin);
  expect(f.calls.at(-1)).toMatchObject({ url: "/api/questions/1/pin", method: "PUT" });
  unmount();
  renderWithApp(<QuestionTable rows={[row({ pinned: true })]} />);
  await userEvent.click(await screen.findByRole("button", { name: /pin/i }));
  expect(f.calls.at(-1)).toMatchObject({ url: "/api/questions/1/pin", method: "DELETE" });
});

it("long_prompt_is_truncated_with_title", async () => {
  const prompt = `${"Why does a very long question prompt keep going ".repeat(8)}?`;
  renderWithApp(<QuestionTable rows={[row({ prompt })]} />);
  const link = await screen.findByRole("link", { name: prompt });
  expect(link).toHaveAttribute("title", prompt);
  expect(link.className).toContain("truncate");
  expect(link).toHaveAttribute("href", "/questions/1");
});
