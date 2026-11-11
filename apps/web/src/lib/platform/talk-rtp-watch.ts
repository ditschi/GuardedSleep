import { TALK_RTP_GRACE_MS } from "./policy";

/**
 * Baby: notices talk RTP that arrives while no playback lease is open (`talk_rtp_outside_grant`).
 * It reads only the packet counter of the talk receiver, never its content. A parent whose last
 * frames are still in flight when a lease ends is not a breach, so the counter is also ignored for
 * a short grace after the lease closed.
 */
export class TalkRtpWatch {
  private last: number | null = null;
  private leaseSeenAt: number | null = null;

  /** True when packets arrived since the previous call although no lease was open (or just was). */
  observe(packetsReceived: number, leaseOpen: boolean, nowMs: number): boolean {
    if (leaseOpen) this.leaseSeenAt = nowMs;
    const previous = this.last;
    this.last = packetsReceived;
    if (previous === null || packetsReceived <= previous) return false;
    if (leaseOpen) return false;
    return this.leaseSeenAt === null || nowMs - this.leaseSeenAt > TALK_RTP_GRACE_MS;
  }

  /** A new attempt starts counting from zero. */
  reset(): void {
    this.last = null;
    this.leaseSeenAt = null;
  }
}
