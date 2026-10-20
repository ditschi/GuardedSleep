/** In-memory token buckets keyed by client address; keys are never logged. */
export class RateLimiter {
  private readonly buckets = new Map<string, { tokens: number; at: number }>();
  private readonly capacity: number;
  private readonly refillPerMs: number;

  /** @param perMinute tokens per minute and burst size; 0 disables the limiter. */
  constructor(
    perMinute: number,
    private readonly now: () => number = Date.now,
  ) {
    this.capacity = perMinute;
    this.refillPerMs = perMinute / 60_000;
  }

  /** Consumes one token; false when the key is over its limit. */
  take(key: string): boolean {
    if (this.capacity <= 0) return true;
    const t = this.now();
    const b = this.buckets.get(key) ?? { tokens: this.capacity, at: t };
    b.tokens = Math.min(this.capacity, b.tokens + (t - b.at) * this.refillPerMs);
    b.at = t;
    const ok = b.tokens >= 1;
    if (ok) b.tokens -= 1;
    this.buckets.set(key, b);
    return ok;
  }

  /** Drops buckets that have refilled completely so the map cannot grow without bound. */
  prune(): void {
    const t = this.now();
    for (const [k, b] of this.buckets) {
      if (b.tokens + (t - b.at) * this.refillPerMs >= this.capacity) this.buckets.delete(k);
    }
  }
}
