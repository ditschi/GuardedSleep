/**
 * Two clocks, never mixed: the core and all durations use the monotonic `elapsedMs`
 * (`performance.now()`); only user-visible clock times use `wallMs`.
 */
export interface Clock {
  elapsedMs(): number;
  wallMs(): number;
}

export function createClock(
  perf: { now(): number } = performance,
  wall: () => number = Date.now,
): Clock {
  let last = 0;
  return {
    // Whole milliseconds, never going backwards: the core requires a monotonic `now_ms`.
    elapsedMs() {
      const now = Math.floor(perf.now());
      if (now > last) last = now;
      return last;
    },
    wallMs: wall,
  };
}

/** Cancels a scheduled timer. */
export type Cancel = () => void;

/**
 * Timer access of the platform layer. Timers only feed inputs to the core (tick, audio level,
 * status polling) or flush a stop; no timer decides anything about the link.
 */
export interface Timers {
  every(fn: () => void, ms: number): Cancel;
  after(fn: () => void, ms: number): Cancel;
}

export function createTimers(): Timers {
  return {
    every(fn, ms) {
      const id = setInterval(fn, ms);
      return () => clearInterval(id);
    },
    after(fn, ms) {
      const id = setTimeout(fn, ms);
      return () => clearTimeout(id);
    },
  };
}
