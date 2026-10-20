/**
 * Monotonic millisecond clock for the core: `Date.now()` clamped so it never goes backwards (the
 * core requires monotonic input; a wall-clock jump back would otherwise stall its timers).
 */
export function createMonotonicClock(wall: () => number = Date.now): () => number {
  let last = 0;
  return () => {
    const t = wall();
    if (Number.isFinite(t) && t > last) last = t;
    return last;
  };
}

export type StopTicker = () => void;

export type TickerFactory = (onTick: () => void, intervalMs: number) => StopTicker;

/**
 * Ticks from a dedicated Worker: browsers throttle main-thread timers in hidden tabs to once per
 * second or even once per minute, but a Worker's timers keep running (ADR-017: the core wants a
 * tick at least every 500 ms). Falls back to `setInterval` where Workers are unavailable.
 */
export const workerTicker: TickerFactory = (onTick, intervalMs) => {
  if (typeof Worker === "undefined" || typeof Blob === "undefined" || typeof URL === "undefined") {
    const id = setInterval(onTick, intervalMs);
    return () => clearInterval(id);
  }
  let worker: Worker | null = null;
  let url: string | null = null;
  try {
    const src = `setInterval(function () { postMessage(0); }, ${Math.max(1, Math.round(intervalMs))});`;
    url = URL.createObjectURL(new Blob([src], { type: "text/javascript" }));
    worker = new Worker(url);
    worker.onmessage = () => onTick();
  } catch {
    if (url) URL.revokeObjectURL(url);
    const id = setInterval(onTick, intervalMs);
    return () => clearInterval(id);
  }
  return () => {
    worker?.terminate();
    if (url) URL.revokeObjectURL(url);
  };
};
