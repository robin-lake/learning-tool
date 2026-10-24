import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { mockFetch, renderWithApp } from "@/test/render";
import { RatingBox } from "./RatingBox";

afterEach(() => vi.unstubAllGlobals());

it("submit_disabled_until_star_then_calls_mutation", async () => {
  const f = mockFetch(201, { id: 7 });
  vi.stubGlobal("fetch", f.fn);
  renderWithApp(<RatingBox questionId={2} ratings={[]} />);
  const submit = await screen.findByRole("button", { name: "Save rating" });
  expect(submit).toBeDisabled();
  await userEvent.click(screen.getByRole("radio", { name: "4 of 5" }));
  await userEvent.type(screen.getByLabelText(/why/i), "clear and useful");
  await userEvent.click(submit);
  expect(f.calls.at(-1)).toMatchObject({ url: "/api/questions/2/ratings", method: "POST" });
  expect(JSON.parse(f.calls.at(-1)?.body ?? "{}")).toEqual({ usefulness: 4, feedback: "clear and useful" });
});
