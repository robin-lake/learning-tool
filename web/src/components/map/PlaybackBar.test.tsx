import { act, fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { afterEach, expect, it, vi } from "vitest";
import { PlaybackBar } from "./PlaybackBar";

const days = Array.from({ length: 11 }, (_, i) => `2026-10-${String(i + 1).padStart(2, "0")}`);

function Harness({ start = null as number | null, list = days }) {
  const [index, setIndex] = useState<number | null>(start);
  return <PlaybackBar days={list} index={index} onIndex={setIndex} />;
}

const slider = () => screen.getByRole("slider", { name: "Day" }) as HTMLInputElement;

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

it("step_buttons_move_one_day", () => {
  render(<Harness />);
  expect(screen.getByText("Today")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Previous day" }));
  expect(slider().value).toBe("9");
  expect(screen.getByText("10 Oct 2026")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Next day" }));
  expect(screen.getByText("Today")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Next day" })).toBeDisabled();
});

it("play_advances_by_elapsed_time", () => {
  vi.useFakeTimers({
    toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance", "setTimeout"],
  });
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: "Play" }));
  expect(slider().value).toBe("0");
  act(() => vi.advanceTimersByTime(1100));
  expect(slider().value).toBe("2");
  act(() => vi.advanceTimersByTime(4000));
  expect(screen.getByText("Today")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
});

it("playback_uses_elapsed_time", () => {
  // One late frame after 2.5 s must land mid-way, not one step along.
  let now = 0;
  vi.spyOn(performance, "now").mockImplementation(() => now);
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: "Play" }));
  now = 2500;
  act(() => frames.shift()?.(now));
  expect(slider().value).toBe("5");
});

it("escape_stops_playback", () => {
  vi.useFakeTimers({
    toFake: ["requestAnimationFrame", "cancelAnimationFrame", "performance", "setTimeout"],
  });
  render(<Harness />);
  fireEvent.click(screen.getByRole("button", { name: "Play" }));
  act(() => vi.advanceTimersByTime(1100));
  fireEvent.keyDown(window, { key: "Escape" });
  act(() => vi.advanceTimersByTime(2000));
  expect(slider().value).toBe("2");
  expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
});

it("empty_history_disables_playback", () => {
  render(<Harness list={[]} />);
  expect(screen.getByText("No history yet")).toBeInTheDocument();
  for (const name of ["Previous day", "Play", "Next day"]) {
    expect(screen.getByRole("button", { name })).toBeDisabled();
  }
  expect(slider()).toBeDisabled();
});
