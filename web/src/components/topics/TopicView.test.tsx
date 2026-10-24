import { screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { TopicPage } from "@/api/generated/TopicPage";
import { mockFetch, renderWithApp } from "@/test/render";
import { card, row, topic } from "./fixtures";
import { TopicView } from "./TopicView";

afterEach(() => vi.unstubAllGlobals());

it("header_counts_subtree_active_questions", async () => {
  const page: TopicPage = {
    topic,
    breadcrumbs: [{ id: 4, name: "Impedance" }],
    subtree_active_questions: 6,
    children: [card],
    questions: [row(), row({ id: 2 })],
    links: [],
    references: [],
  };
  vi.stubGlobal("fetch", mockFetch(200, page).fn);
  renderWithApp(<TopicView id={4} search={{ all: true }} />);
  expect(await screen.findByText(/6 active questions here and below/)).toBeInTheDocument();
});
