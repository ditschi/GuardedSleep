import { readable, type Readable } from "svelte/store";

/** Both clocks the screens read for what they show: monotonic for durations, wall for times. */
export interface Instant {
  elapsedMs: number;
  wallMs: number;
}

let elapsedSource: () => number = () => performance.now();

/**
 * Where the monotonic reading comes from. `main.ts` hands over the platform clock, which adds the
 * time a sleeping laptop did not count, so the screens and the controller agree on "now".
 */
export function setElapsedSource(source: () => number): void {
  elapsedSource = source;
}

export function instantNow(): Instant {
  return { elapsedMs: elapsedSource(), wallMs: Date.now() };
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
