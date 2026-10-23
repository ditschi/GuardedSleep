import type { LinkEvent, LinkStatus } from "@guardedsleep/core-wasm";
import { defaultBabyMonitor, defaultLinkUi, type BabyMonitorState } from "../contract";
import type { Clock } from "./clock";
import { linkUiOf } from "./status-mapping";

/** Same window as the core's `mic_silent` (30 s of digital silence). */
const MIC_SILENT_MS = 30_000;

/**
 * Maps the core's status onto `BabyMonitorState`. The baby never alerts, so there is no sink.
 * Every change is pushed to `onChange`.
 */
export class BabyModel {
  private last: LinkStatus | null = null;
  private micLevel = 0;
  private zeroSince: number | null = null;
  private state: BabyMonitorState = defaultBabyMonitor();

  constructor(
    private readonly clock: Clock,
    private readonly onChange: (state: BabyMonitorState) => void,
  ) {}

  /** Forget the run; only the gate settings survive a restart of the link. */
  reset(): void {
    this.last = null;
    this.micLevel = 0;
    this.zeroSince = null;
    this.state = {
      ...defaultBabyMonitor(),
      threshold: this.state.threshold,
      holdSeconds: this.state.holdSeconds,
    };
    this.onChange(this.state);
  }

  onStatus(status: LinkStatus): void {
    this.last = status;
    this.publish(this.clock.elapsedMs());
  }

  onEvent(event: LinkEvent): void {
    switch (event.type) {
      case "noise_gate_changed":
        this.setSettings(event.threshold, Math.floor(event.hold_ms / 1000));
        break;
      // The parent's listen request reaches the baby as this echo.
      case "listen_changed":
        this.update({ parentListening: event.enabled });
        break;
      default:
        break;
    }
  }

  onMicLevel(rms: number, now: number): void {
    this.micLevel = Math.min(1, Math.max(0, rms));
    this.zeroSince = rms === 0 ? (this.zeroSince ?? now) : null;
    this.publish(now);
  }

  setSettings(threshold: number, holdSeconds: number): void {
    this.update({ threshold, holdSeconds });
  }

  setKioskLocked(locked: boolean): void {
    this.update({ kioskLocked: locked });
  }

  setSilenceRestoredNotice(show: boolean): void {
    this.update({ silenceRestoredNotice: show });
  }

  /** The run could not start at all (microphone refused): a terminal failure with a key. */
  fail(failureKey: string): void {
    this.update({
      link: { ...defaultLinkUi(), phase: "failed_needs_user", failureKey },
    });
  }

  private update(patch: Partial<BabyMonitorState>): void {
    this.state = { ...this.state, ...patch };
    this.onChange(this.state);
  }

  private publish(now: number): void {
    const s = this.last;
    const silent = this.zeroSince !== null && now - this.zeroSince >= MIC_SILENT_MS;
    this.update({
      link: s ? linkUiOf(s, "unknown") : defaultLinkUi(),
      micLevel: this.micLevel,
      micSilent: silent,
    });
  }
}
