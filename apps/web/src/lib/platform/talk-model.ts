import type { DeviceStatus, LinkEvent } from "@guardedsleep/core-wasm";
import {
  defaultParentTalkFields,
  type BabyTalkFields,
  type NoisePause,
  type ParentTalkFields,
  type TalkDeny,
  type TalkEnd,
  type TalkPermission,
  type TalkUi,
  type TalkUnavailable,
} from "../contract";
import type { AlertSink } from "./alert-sink";
import { tonePlanForLink } from "./audio";
import type { Clock } from "./clock";
import {
  TALK_AFTER_LISTEN_MS,
  TALK_MIC_SILENT_BELOW,
  TALK_MIC_SILENT_MS,
  TALK_RESULT_VISIBLE_MS,
  TALK_UNCONFIRMED_VISIBLE_MS,
} from "./policy";

/** The microphone step as the controller knows it; the contract shows `none` for the first two. */
export type MicStep = "unknown" | "granted" | "sheet" | "done" | "refused";

const PERMISSION_FOR_STEP: Record<MicStep, TalkPermission> = {
  unknown: "none",
  granted: "none",
  sheet: "needed",
  done: "just_granted",
  refused: "refused",
};

type TalkPhase = "idle" | "requesting" | "talking" | "denied";

/**
 * Maps the core's talk events and the baby's status onto what the parent's talk button shows (the
 * talk fields of `ParentMonitorState`). It decides nothing: whether a talk may start, how long it
 * runs and why it ended are the core's (`talk_changed`, `peer_status`); this only turns them into
 * one UI model. Every change that differs from the last one is pushed to `onChange`.
 */
export class ParentTalkModel {
  private phase: TalkPhase = "idle";
  private until: number | null = null;
  private ended: TalkEnd | null = null;
  private denied: TalkDeny | null = null;
  private step: MicStep = "unknown";
  private unconfirmed = false;
  private micQuietSince: number | null = null;
  private micSilent = false;
  private afterListenUntil: number | null = null;
  private connected = false;
  private browserSupported = true;
  private peer: DeviceStatus | null = null;
  private paused = false;
  private resultUntil: number | null = null;
  private retryAt: number | null = null;
  private unconfirmedPending = false;
  private unconfirmedUntil: number | null = null;
  private endedAt: number | null = null;
  private lastJson = "";

  constructor(
    private readonly clock: Clock,
    private readonly onChange: (fields: ParentTalkFields) => void,
  ) {}

  get state(): ParentTalkFields {
    return this.build(this.clock.elapsedMs());
  }

  /** The run ended: forget the talk, keep what belongs to the browser (support, permission). */
  reset(): void {
    this.phase = "idle";
    this.until = null;
    this.ended = null;
    this.denied = null;
    this.step = this.step === "sheet" ? "unknown" : this.step;
    this.unconfirmed = false;
    this.clearMicSilent();
    this.afterListenUntil = null;
    this.connected = false;
    this.peer = null;
    this.paused = false;
    this.resultUntil = null;
    this.retryAt = null;
    this.unconfirmedPending = false;
    this.unconfirmedUntil = null;
    this.endedAt = null;
    this.publish(this.clock.elapsedMs());
  }

  setBrowserSupported(supported: boolean): void {
    this.browserSupported = supported;
    this.publish(this.clock.elapsedMs());
  }

  setConnected(connected: boolean): void {
    this.connected = connected;
    this.publish(this.clock.elapsedMs());
  }

  setPermission(step: MicStep): void {
    this.step = step;
    this.publish(this.clock.elapsedMs());
  }

  get permission(): MicStep {
    return this.step;
  }

  /** The talk button can be pressed right now (the same rule the screen uses to enable it). */
  get available(): boolean {
    return this.build(this.clock.elapsedMs()).talk.kind !== "unavailable";
  }

  get busy(): boolean {
    return this.phase === "requesting" || this.phase === "talking";
  }

  /** The parent chose "keep listening": the after-talk line is no longer needed. */
  clearAfterListen(): void {
    this.afterListenUntil = null;
    this.publish(this.clock.elapsedMs());
  }

  /**
   * The level of the parent's own microphone while a talk has it attached. Presentation only:
   * nothing here reaches the core, and the line stays visual (the screen never speaks it).
   */
  onMicLevel(rms: number, now: number): void {
    if (this.phase !== "talking") return;
    if (rms >= TALK_MIC_SILENT_BELOW) {
      this.clearMicSilent();
    } else {
      this.micQuietSince ??= now;
      this.micSilent = now - this.micQuietSince >= TALK_MIC_SILENT_MS;
    }
    this.publish(now);
  }

  private clearMicSilent(): void {
    this.micQuietSince = null;
    this.micSilent = false;
  }

  onEvent(event: LinkEvent, now: number): void {
    switch (event.type) {
      case "talk_changed":
        this.onTalkChanged(event.state, now);
        break;
      case "noise_paused":
        this.paused = event.paused;
        break;
      case "talk_playback":
        this.onPlayback(event.result, now);
        break;
      case "peer_status":
        this.onPeerStatus(event.status, now);
        break;
      case "listen_changed":
        // The after-talk listen starts in the same step in which the talk ended; its end arrives
        // as `enabled: false`.
        if (event.enabled) {
          if (this.endedAt === now && !this.busy)
            this.afterListenUntil = now + TALK_AFTER_LISTEN_MS;
        } else {
          this.afterListenUntil = null;
        }
        break;
      default:
        break;
    }
    this.publish(now);
  }

  onTick(now: number): void {
    if (this.resultUntil !== null && now >= this.resultUntil) {
      this.resultUntil = null;
      this.phase = "idle";
      this.ended = null;
      this.denied = null;
    }
    if (this.unconfirmedUntil !== null && now >= this.unconfirmedUntil) {
      this.unconfirmedUntil = null;
      this.unconfirmedPending = false;
      this.unconfirmed = false;
    }
    this.publish(now);
  }

  private onTalkChanged(
    state: Extract<LinkEvent, { type: "talk_changed" }>["state"],
    now: number,
  ): void {
    switch (state.state) {
      case "requesting":
        this.resultUntil = null;
        this.unconfirmedPending = false;
        this.unconfirmedUntil = null;
        this.phase = "requesting";
        this.until = null;
        this.ended = null;
        this.denied = null;
        this.unconfirmed = false;
        this.clearMicSilent();
        this.afterListenUntil = null;
        break;
      case "talking":
        this.phase = "talking";
        this.until = state.until_ms;
        break;
      case "ended": {
        const reason: TalkEnd = state.reason;
        this.endedAt = now;
        this.releaseUnconfirmed(now);
        this.phase = "idle";
        this.until = null;
        this.clearMicSilent();
        this.ended = reason;
        // A plain release needs no message; the result of the playback check may follow.
        this.resultUntil =
          reason === "released" || reason === "stopped" ? null : now + TALK_RESULT_VISIBLE_MS;
        break;
      }
      case "denied": {
        const denied: TalkDeny = state.reason?.kind ?? "unknown";
        if (
          state.reason !== null &&
          (state.reason.kind === "cooldown" || state.reason.kind === "budget")
        ) {
          this.retryAt = now + state.reason.retry_in_ms;
        }
        this.endedAt = null;
        this.resultUntil = now + TALK_RESULT_VISIBLE_MS;
        this.phase = "denied";
        this.until = null;
        this.denied = denied;
        break;
      }
    }
  }

  private onPlayback(result: "heard" | "unconfirmed" | "unknown", now: number): void {
    if (result === "unconfirmed") {
      this.unconfirmedPending = true;
      this.unconfirmedUntil = this.busy ? null : now + TALK_UNCONFIRMED_VISIBLE_MS;
      this.unconfirmed = true;
    } else if (result === "heard") {
      this.unconfirmedPending = false;
      this.unconfirmedUntil = null;
      this.unconfirmed = false;
    }
  }

  /** The "may not be audible" line counts from the end of the talk, not from the check. */
  private releaseUnconfirmed(now: number): void {
    if (this.unconfirmedPending && this.unconfirmedUntil === null) {
      this.unconfirmedUntil = now + TALK_UNCONFIRMED_VISIBLE_MS;
    }
  }

  private onPeerStatus(status: DeviceStatus, now: number): void {
    this.peer = status;
    this.paused = status.noise_paused === true;
    // The status is the baby's current word on the cooldown and the budget.
    this.retryAt =
      status.talk_retry_in_ms !== undefined && status.talk_retry_in_ms > 0
        ? now + status.talk_retry_in_ms
        : null;
  }

  private unavailable(now: number): TalkUnavailable | null {
    // A browser that cannot capture reads as "this device can't talk"; the screen words it.
    if (!this.browserSupported) return "unsupported";
    if (!this.connected) return "not_connected";
    const peer = this.peer;
    if (peer?.talk === "off") return "off";
    if (peer?.talk === "unsupported") return "unsupported";
    if (peer?.talk === "muted") return "muted";
    if (peer?.talker === "other") return "other_talking";
    if (this.retryAt !== null && now < this.retryAt) return "cooldown";
    return null;
  }

  private talkUi(now: number): TalkUi {
    const pending = this.retryAt !== null && now < this.retryAt ? this.retryAt : null;
    switch (this.phase) {
      case "requesting":
        return { kind: "requesting" };
      case "talking":
        return { kind: "talking", untilElapsedMs: this.until ?? now };
      case "denied": {
        const reason = this.denied ?? "unknown";
        const waits = reason === "cooldown" || reason === "budget";
        return { kind: "denied", reason, retryAtElapsedMs: waits ? pending : null };
      }
      case "idle": {
        const reason = this.unavailable(now);
        return reason === null
          ? { kind: "idle" }
          : {
              kind: "unavailable",
              reason,
              retryAtElapsedMs: reason === "cooldown" ? pending : null,
            };
      }
    }
  }

  private build(now: number): ParentTalkFields {
    const self = this.busy || this.peer?.talker === "you";
    const noisePause: NoisePause | null = this.paused ? (self ? "you" : "other") : null;
    return {
      ...defaultParentTalkFields(),
      talk: this.talkUi(now),
      talkEnd: this.ended,
      talkPermission: PERMISSION_FOR_STEP[this.step],
      noisePause,
      talkUnconfirmed: this.unconfirmed,
      talkAudibleUncertain: this.peer?.talk_audible === "uncertain",
      talkMicSilent: this.micSilent,
      afterTalkListenUntilElapsedMs: this.afterListenUntil,
    };
  }

  private publish(now: number): void {
    const next = this.build(now);
    const json = JSON.stringify(next);
    if (json === this.lastJson) return;
    this.lastJson = json;
    this.onChange(next);
  }
}

/**
 * The baby device's talk-back view: the indicator while a parent's voice plays and the "last talk"
 * line (local only), merged into `BabyMonitorState` by the controller.
 */
export class BabyTalkModel {
  private fields: BabyTalkFields = { parentTalking: false, lastTalk: null };

  constructor(
    private readonly clock: Clock,
    private readonly parentName: () => string,
    private readonly onChange: (fields: BabyTalkFields) => void,
  ) {}

  get state(): BabyTalkFields {
    return this.fields;
  }

  /** A new run: no talk plays, the last talk of the page stays. */
  reset(): void {
    this.update({ parentTalking: false });
  }

  onEvent(event: LinkEvent): void {
    if (event.type !== "talk_active") return;
    if (event.active) {
      this.update({ parentTalking: true });
      return;
    }
    const endWall = this.clock.wallMs();
    this.update({
      parentTalking: false,
      lastTalk: {
        name: this.parentName(),
        atWallMs: endWall - event.duration_ms,
        durationMs: event.duration_ms,
      },
    });
  }

  private update(patch: Partial<BabyTalkFields>): void {
    this.fields = { ...this.fields, ...patch };
    this.onChange(this.fields);
  }
}

/**
 * Wraps the alert sink: whenever an alert is about to make a sound on the parent device, a talk in
 * progress ends first (`alert_preempts_talk`, spec `talk-back` "Alerts take precedence over talk").
 * The wrapper adds no sound and removes none.
 */
export function preemptingAlerts(inner: AlertSink, onSound: () => void): AlertSink {
  return {
    linkState(snapshot) {
      const plan = tonePlanForLink(snapshot);
      if (plan.toneEveryMs !== null || plan.continuousAlarm) onSound();
      inner.linkState(snapshot);
    },
    linkLost: (downMs) => inner.linkLost(downMs),
    noiseAlert(kind, probe, snoozed) {
      if (!snoozed) onSound();
      inner.noiseAlert(kind, probe, snoozed);
    },
    noiseEnded: () => inner.noiseEnded(),
    stopNoiseSound: () => inner.stopNoiseSound(),
    notice(notice) {
      if (notice.type === "peer_stopped") onSound();
      inner.notice(notice);
    },
    clear: () => inner.clear(),
  };
}
