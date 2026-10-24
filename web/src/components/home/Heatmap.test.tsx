import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { Heatmap } from "./Heatmap";

const wrap = (ui: React.ReactNode) => render(<TooltipProvider delayDuration={0}>{ui}</TooltipProvider>);

it("heatmap_renders_empty_year", () => {
  wrap(<Heatmap activity={[]} today="2026-10-24" />);
  const cells = screen.getAllByRole("button");
  expect(cells.length).toBe(371);
  expect(cells[cells.length - 1]).toHaveAccessibleName("24 Oct 2026: no questions");
});

it("tooltip_shows_grades_and_topics_on_focus", async () => {
  wrap(
    <Heatmap
      activity={[
        {
          study_day: "2026-10-24",
          count: 5,
          by_grade: [3, 1, 0, 1],
          topics: [
            { id: 1, name: "AC basics" },
            { id: 2, name: "HornResp" },
          ],
        },
      ]}
      today="2026-10-24"
    />,
  );
  const cell = screen.getByRole("button", { name: "24 Oct 2026: 5 questions" });
  fireEvent.focus(cell);
  expect((await screen.findAllByText("Easy 1 · Good 0 · Hard 1 · Again 3")).length).toBeGreaterThan(0);
  expect(screen.getAllByText("Topics: AC basics, HornResp").length).toBeGreaterThan(0);
});

it("scrolls_to_show_today", () => {
  const setScroll = vi.spyOn(Element.prototype, "scrollLeft", "set");
  vi.spyOn(Element.prototype, "scrollWidth", "get").mockReturnValue(900);
  wrap(<Heatmap activity={[]} today="2026-10-24" />);
  expect(setScroll).toHaveBeenCalledWith(900);
  vi.restoreAllMocks();
});

it("does_not_rescroll_on_rerender", () => {
  const setScroll = vi.spyOn(Element.prototype, "scrollLeft", "set");
  const { rerender } = wrap(<Heatmap activity={[]} today="2026-10-24" />);
  rerender(
    <TooltipProvider delayDuration={0}>
      <Heatmap activity={[]} today="2026-10-24" />
    </TooltipProvider>,
  );
  expect(setScroll).toHaveBeenCalledTimes(1);
  vi.restoreAllMocks();
});
