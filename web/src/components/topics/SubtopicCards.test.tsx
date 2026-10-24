import { screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { renderWithApp } from "@/test/render";
import { card } from "./fixtures";
import { SubtopicCards } from "./SubtopicCards";

it("card_links_and_progress", async () => {
  renderWithApp(<SubtopicCards cards={[card]} />);
  expect(await screen.findByRole("link", { name: /Loudspeaker impedance/ })).toHaveAttribute(
    "href",
    "/topics/5",
  );
  expect(screen.getByText(/3 of 4 studied/)).toBeInTheDocument();
  expect(screen.getByText(/recall 82%/)).toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "75");
});
