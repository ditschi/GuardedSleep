/**
 * The last minute of the parent's sound strip (`parent-alerts` "Monitor sound strip"): the live
 * level, sampled once per second, folded into 60 bars of one second each. Presentation only: no
 * alert decision reads it (the core and the noise tracker decide). The Android client has the
 * identical class (`LevelHistory.kt`) and the same test cases.
 */
interface Sample {
  atMs: number;
  level: number;
}

export class LevelHistory {
  private samples: Sample[] = [];

  constructor(
    private readonly windowMs = 60_000,
    private readonly stepMs = 1_000,
  ) {}

  /** Records a level (clamped to 0..1); samples older than twice the window are dropped. */
  push(nowMs: number, level: number): void {
    const clamped = Number.isFinite(level) ? Math.min(1, Math.max(0, level)) : 0;
    this.samples.push({ atMs: nowMs, level: clamped });
    const oldest = nowMs - 2 * this.windowMs;
    if (this.samples[0] !== undefined && this.samples[0].atMs < oldest) {
      this.samples = this.samples.filter((s) => s.atMs >= oldest);
    }
  }

  /** Oldest bar first; each bar keeps the maximum of its samples, empty bars are 0. */
  bars(nowMs: number, count = this.windowMs / this.stepMs): number[] {
    const bars = new Array<number>(count).fill(0);
    const start = nowMs - this.windowMs;
    const bucketMs = this.windowMs / count;
    for (const s of this.samples) {
      if (s.atMs < start || s.atMs > nowMs) continue;
      const i = Math.min(count - 1, Math.floor((s.atMs - start) / bucketMs));
      if (s.level > (bars[i] ?? 0)) bars[i] = s.level;
    }
    return bars;
  }

  clear(): void {
    this.samples = [];
  }
}
