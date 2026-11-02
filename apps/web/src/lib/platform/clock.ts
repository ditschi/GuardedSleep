/**
 * The core's clock and the wall clock (link-supervision spec, "Clock and time jumps"). `elapsedMs`
 * is `Date.now()` in whole milliseconds, clamped so it never goes backwards: unlike
 * `performance.now()` it keeps counting while a laptop sleeps, so the core sees that time passed
 * and decides by itself (liveness window, escalation). Durations and deadlines use `elapsedMs`;
 * user-visible clock times use `wallMs`. If the wall clock is set back, `elapsedMs` stands still
 * until the wall clock catches up.
 */
export interface Clock {
  elapsedMs(): number;
  wallMs(): number;
}

export function createClock(wall: () => number = Date.now): Clock {
  let last = 0;
  return {
    elapsedMs() {
      const now = Math.floor(wall());
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
