import { readable, type Readable } from "svelte/store";

/** Both clocks the screens read for what they show: monotonic for durations, wall for times. */
export interface Instant {
  elapsedMs: number;
  wallMs: number;
}

export function instantNow(): Instant {
  return { elapsedMs: performance.now(), wallMs: Date.now() };
}

/**
 * A store that re-reads the clock every `intervalMs` while somebody subscribes. Countdowns and
 * "for 2 min" texts use it; no timer here decides anything about the link.
 */
export function createTicker(intervalMs: number): Readable<Instant> {
  return readable(instantNow(), (set) => {
    set(instantNow());
    const id = setInterval(() => set(instantNow()), intervalMs);
    return () => clearInterval(id);
  });
}

/** One second: enough for a countdown and a duration text. */
export const everySecond = createTicker(1000);
