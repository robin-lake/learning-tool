import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { timeline } from "@/lib/map/fixtures";
import { buildTree } from "@/lib/map/tree";
import { MapView } from "./MapView";

const stale = { after: 14, full: 60 };

function draw(rootId: string | null, size = 400, handlers = { onZoom: vi.fn(), onOpenQuestion: vi.fn() }) {
  const root = buildTree(timeline.nodes, rootId, null, "fixed");
  if (!root) throw new Error("no tree");
  render(
    <TooltipProvider delayDuration={0}>
      <MapView
        root={root}
        width={size}
        height={size}
        color="combined"
        stale={stale}
        day="2026-10-03"
        {...handlers}
      />
    </TooltipProvider>,
  );
  return handlers;
}

it("renders_a_circle_per_child_with_accessible_name", () => {
  draw(null);
  const circles = screen.getAllByRole("button");
  expect(circles).toHaveLength(2);
  expect(circles[0]).toHaveAccessibleName("Electric circuits: 2 questions, recall 80%");
  expect(circles[1]).toHaveAccessibleName("Speaker design tools: 1 question, recall 80%");
});

it("click_topic_calls_onZoom", () => {
  const h = draw(null);
  fireEvent.click(screen.getByRole("button", { name: /^Electric circuits/ }));
  expect(h.onZoom).toHaveBeenCalledWith("t1");
  expect(h.onOpenQuestion).not.toHaveBeenCalled();
});

it("enter_on_question_opens_it", () => {
  const h = draw("t1");
  fireEvent.keyDown(screen.getByRole("button", { name: /^q1/ }), { key: "Enter" });
  expect(h.onOpenQuestion).toHaveBeenCalledWith(1);
  expect(h.onZoom).not.toHaveBeenCalled();
});

it("label_hidden_when_it_does_not_fit", () => {
  draw(null, 80);
  expect(screen.queryByText("Electric circuits")).toBeNull();
  expect(screen.getByRole("button", { name: /^Electric circuits: 2 questions/ })).toBeInTheDocument();
});

it("shows_label_when_it_fits", () => {
  draw(null);
  expect(screen.getByText("Electric circuits")).toBeInTheDocument();
});

it("never_studied_uses_hatch_pattern", () => {
  draw("t1");
  const unstudied = screen.getByRole("button", { name: "q2: 1 question, not studied" });
  expect(unstudied.getAttribute("fill")).toMatch(/^url\(#/);
  const studied = screen.getByRole("button", { name: /^q1/ });
  expect(studied.getAttribute("fill")).toMatch(/^oklch\(/);
});

it("new_circles_grow_in_from_zero", () => {
  draw(null);
  // Animation doesn't run in jsdom: the first frame shows where each circle starts.
  for (const c of screen.getAllByRole("button")) expect(c.getAttribute("r")).toBe("0");
});
