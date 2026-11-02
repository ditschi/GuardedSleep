import type { NoiseKind } from "./alert-sink";
import { NOISE_EVENT_JOIN_MS, NOISE_QUIET_AFTER_MS } from "./policy";

/**
 * Lifecycle of a noise event on the parent (alpha-ux-flows 3.7): a new full alert only for noise
 * at least 2 minutes after the previous event ended; noise in between continues that event. The
 * event ends after 2 minutes without loud audio. It only shapes what the parent sees and hears;
 * whether a noise alert exists at all is the core's decision.
 */
export class NoiseTracker {
  active = false;
  lastLoudAt = 0;
  private endedAt: number | null = null;

  onAlert(now: number): NoiseKind {
    const joins =
      this.active || (this.endedAt !== null && now - this.endedAt < NOISE_EVENT_JOIN_MS);
    this.active = true;
    this.endedAt = null;
    this.lastLoudAt = now;
    return joins ? "continue" : "new";
  }

  onLevel(now: number, level: number, threshold: number): void {
    if (this.active && level >= threshold) this.lastLoudAt = now;
  }

  /** True once, when the event just ended. */
  tick(now: number): boolean {
    if (this.active && now - this.lastLoudAt >= NOISE_QUIET_AFTER_MS) {
      this.active = false;
      this.endedAt = now;
      return true;
    }
    return false;
  }

  reset(): void {
    this.active = false;
    this.endedAt = null;
  }
}
