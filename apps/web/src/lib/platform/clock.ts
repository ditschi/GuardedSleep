/**
 * Two clocks, never mixed: the core and all durations use the monotonic `elapsedMs`
 * (`performance.now()`); only user-visible clock times use `wallMs`.
 */
export interface Clock {
  elapsedMs(): number;
  wallMs(): number;
}

/**
 * A suspended laptop can stop `performance.now()` (the monotonic clock) while the wall clock keeps
 * going, so the core would never learn that time passed. A gap between the two clocks larger than
 * this is read as "the device slept".
 */
export const SLEEP_GAP_MS = 10_000;

/** The device slept: `gapMs` of time the monotonic clock did not count, ending at `wallMs`. */
export interface SleepGap {
  gapMs: number;
  wallMs: number;
}

export interface SleepAwareClock extends Clock {
  /** Called when a sleep gap was added to the clock. Returns the unsubscribe function. */
  onGap(fn: (gap: SleepGap) => void): () => void;
  /** Reads both clocks now, so a wake-up is noticed before the next timer tick. */
  check(): void;
}

/**
 * Compares how far the wall clock and the monotonic clock moved since the last look. When the wall
 * clock moved at least `thresholdMs` more, that surplus is the sleep gap; the monotonic clock then
 * gains it, so the core sees the elapsed time it would otherwise miss and decides the rest itself
 * (this is only a clock fix, no alarm decision). The wall clock moving backwards is ignored.
 */
export class SleepDetector {
  private lastPerf: number | null = null;
  private lastWall = 0;

  constructor(private readonly thresholdMs = SLEEP_GAP_MS) {}

  /** `perfMs` must already include earlier corrections. Returns the new gap in ms, or 0. */
  observe(perfMs: number, wallMs: number): number {
    const prevPerf = this.lastPerf;
    const prevWall = this.lastWall;
    this.lastPerf = perfMs;
    this.lastWall = wallMs;
    if (prevPerf === null) return 0;
    const surplus = wallMs - prevWall - (perfMs - prevPerf);
    return surplus >= this.thresholdMs ? Math.floor(surplus) : 0;
  }
}

export function createClock(
  perf: { now(): number } = performance,
  wall: () => number = Date.now,
  thresholdMs: number = SLEEP_GAP_MS,
): SleepAwareClock {
  let last = 0;
  let offset = 0;
  const detector = new SleepDetector(thresholdMs);
  const listeners = new Set<(gap: SleepGap) => void>();
  const read = (): number => {
    const perfNow = perf.now();
    const wallNow = wall();
    const gap = detector.observe(perfNow + offset, wallNow);
    if (gap > 0) {
      offset += gap;
      // Restart the baseline on the corrected reading, so the next look sees no second gap.
      detector.observe(perfNow + offset, wallNow);
      for (const fn of [...listeners]) fn({ gapMs: gap, wallMs: wallNow });
    }
    return perfNow + offset;
  };
  return {
    // Whole milliseconds, never going backwards: the core requires a monotonic `now_ms`.
    elapsedMs() {
      const now = Math.floor(read());
      if (now > last) last = now;
      return last;
    },
    wallMs: wall,
    onGap(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    check() {
      read();
    },
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
