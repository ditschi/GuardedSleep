import { TALK_RTP_GRACE_MS, TALK_RTP_OUTSIDE_PACKETS } from "./policy";

/**
 * Baby: notices talk RTP that arrives while no playback lease is open (`talk_rtp_outside_grant`).
 * It reads only the packet counter of the talk receiver, never its content. A parent whose last
 * frames are still in flight when a lease ends is not a breach, so the counter is also ignored for
 * a short grace after the lease closed.
 */
export class TalkRtpWatch {
  private baseline: number | null = null;
  private leaseSeenAt: number | null = null;
  private warned = false;

  /**
   * True once per attempt, when more than `TALK_RTP_OUTSIDE_PACKETS` packets arrived since the
   * baseline although no lease was open (or just was). The baseline follows the counter inside a
   * lease and its grace. Same rule as Android's `TalkPlayout.onTalkPackets`.
   */
  observe(packetsReceived: number, leaseOpen: boolean, nowMs: number): boolean {
    if (leaseOpen) this.leaseSeenAt = nowMs;
    const inGrace = this.leaseSeenAt !== null && nowMs - this.leaseSeenAt < TALK_RTP_GRACE_MS;
    if (leaseOpen || inGrace || this.baseline === null) {
      this.baseline = packetsReceived;
      return false;
    }
    if (packetsReceived - this.baseline > TALK_RTP_OUTSIDE_PACKETS && !this.warned) {
      this.warned = true;
      return true;
    }
    return false;
  }

  /** A new attempt starts counting from zero. */
  reset(): void {
    this.baseline = null;
    this.leaseSeenAt = null;
    this.warned = false;
  }
}
