import { format, parseISO } from "date-fns";
import { ChevronLeft, ChevronRight, Pause, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";

/** A full run from the first day to today takes this long, however many days there are. */
const PLAY_MS = 5000;

type Props = {
  days: string[];
  /** Index into `days`; `null` means today. */
  index: number | null;
  /** `live` is true for the frames of a running playback, which callers needn't persist. */
  onIndex(i: number | null, live?: boolean): void;
  /** Called when playback starts and stops (paused, Esc, finished or interrupted). */
  onPlayingChange?(playing: boolean): void;
};

/** Step, scrub or play through the history, one day at a time. */
export function PlaybackBar({ days, index, onIndex, onPlayingChange }: Props) {
  const n = days.length;
  const last = n - 1;
  const at = index ?? last;
  const [playing, setPlaying] = useState(false);
  const onIndexRef = useRef(onIndex);
  onIndexRef.current = onIndex;
  const onPlayingRef = useRef(onPlayingChange);
  onPlayingRef.current = onPlayingChange;
  const started = useRef(false);
  useEffect(() => {
    if (playing === started.current) return;
    started.current = playing;
    onPlayingRef.current?.(playing);
  }, [playing]);
  const startRef = useRef(0);

  const go = (i: number) => onIndex(i >= last ? null : Math.max(0, i));

  // Elapsed time, not frame count, decides the day, so slow frames skip days instead of slowing down.
  useEffect(() => {
    if (!playing || n < 2) return;
    const startProgress = startRef.current / last;
    const t0 = performance.now();
    let shown = startRef.current;
    let frame = 0;
    const tick = () => {
      const progress = Math.min(1, startProgress + (performance.now() - t0) / PLAY_MS);
      const i = Math.floor(progress * last);
      if (i !== shown) {
        shown = i;
        onIndexRef.current(i >= last ? null : i, true);
      }
      if (progress >= 1) setPlaying(false);
      else frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPlaying(false);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("keydown", onKey);
    };
  }, [playing, n, last]);

  const play = () => {
    // From today, replay the whole history; otherwise carry on from the day shown.
    const from = at >= last ? 0 : at;
    startRef.current = from;
    onIndex(from, true);
    setPlaying(true);
  };

  const empty = n === 0;
  const label = empty ? "No history yet" : at >= last ? "Today" : format(parseISO(days[at]), "d MMM yyyy");
  return (
    <div className="flex items-center gap-2">
      <Button
        variant="outline"
        size="icon"
        aria-label="Previous day"
        disabled={empty || at <= 0}
        onClick={() => {
          setPlaying(false);
          go(at - 1);
        }}
      >
        <ChevronLeft />
      </Button>
      <Button
        variant="outline"
        size="icon"
        aria-label={playing ? "Pause" : "Play"}
        disabled={n < 2}
        onClick={() => (playing ? setPlaying(false) : play())}
      >
        {playing ? <Pause /> : <Play />}
      </Button>
      <Button
        variant="outline"
        size="icon"
        aria-label="Next day"
        disabled={empty || at >= last}
        onClick={() => {
          setPlaying(false);
          go(at + 1);
        }}
      >
        <ChevronRight />
      </Button>
      <input
        type="range"
        aria-label="Day"
        aria-valuetext={label}
        min={0}
        max={Math.max(0, last)}
        value={empty ? 0 : at}
        disabled={empty}
        onChange={(e) => {
          setPlaying(false);
          go(Number(e.target.value));
        }}
        className="w-full max-w-md accent-primary"
      />
      <span className="min-w-24 text-sm tabular-nums text-muted-foreground">{label}</span>
    </div>
  );
}
