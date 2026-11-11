import type { DeviceStatus, LinkEvent } from "@guardedsleep/core-wasm";
import {
  defaultBabyTalk,
  defaultParentTalk,
  type BabyTalkUi,
  type ParentTalkUi,
  type TalkBackSettings,
  type TalkDeniedReason,
  type TalkEndedReason,
  type TalkPermission,
  type TalkUnavailable,
} from "../contract";
import type { AlertSink } from "./alert-sink";
import { tonePlanForLink } from "./audio";
import type { Clock } from "./clock";
import { TALK_RESULT_VISIBLE_MS, TALK_UNCONFIRMED_VISIBLE_MS } from "./policy";

/**
 * Maps the core's talk events and the baby's status onto what the parent's talk button shows. It
 * decides nothing: whether a talk may start, how long it runs and why it ended are the core's
 * (`talk_changed`, `peer_status`); this only turns them into one UI model. Every change that
 * differs from the last one is pushed to `onChange`.
 */
export class ParentTalkModel {
  private ui: ParentTalkUi = defaultParentTalk();
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
    private readonly onChange: (state: ParentTalkUi) => void,
  ) {}

  get state(): ParentTalkUi {
    return this.build(this.clock.elapsedMs());
  }

  /** The run ended: forget the talk, keep what belongs to the browser (support, permission). */
  reset(): void {
    this.ui = {
      ...defaultParentTalk(),
      permission: this.ui.permission === "sheet" ? "unknown" : this.ui.permission,
    };
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

  setPermission(permission: TalkPermission): void {
    this.ui = { ...this.ui, permission };
    this.publish(this.clock.elapsedMs());
  }

  get permission(): TalkPermission {
    return this.ui.permission;
  }

  /** The talk button can be pressed right now (the same rule the screen uses to enable it). */
  get available(): boolean {
    return this.build(this.clock.elapsedMs()).unavailable === null;
  }

  get busy(): boolean {
    return this.ui.phase === "requesting" || this.ui.phase === "talking";
  }

  /** The parent chose "keep listening": the after-talk line is no longer needed. */
  clearAfterListen(): void {
    this.ui = { ...this.ui, afterListen: false };
    this.publish(this.clock.elapsedMs());
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
        // The after-talk listen starts in the same step in which the talk ended.
        this.ui = {
          ...this.ui,
          afterListen: event.enabled && this.endedAt === now && !this.busy,
        };
        break;
      default:
        break;
    }
    this.publish(now);
  }

  onTick(now: number): void {
    if (this.resultUntil !== null && now >= this.resultUntil) {
      this.resultUntil = null;
      this.ui = { ...this.ui, phase: "idle", endedReason: null, deniedReason: null };
    }
    if (this.unconfirmedUntil !== null && now >= this.unconfirmedUntil) {
      this.unconfirmedUntil = null;
      this.unconfirmedPending = false;
      this.ui = { ...this.ui, unconfirmed: false };
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
        this.ui = {
          ...this.ui,
          phase: "requesting",
          untilElapsedMs: null,
          endedReason: null,
          deniedReason: null,
          unconfirmed: false,
          afterListen: false,
        };
        break;
      case "talking":
        this.ui = { ...this.ui, phase: "talking", untilElapsedMs: state.until_ms };
        break;
      case "ended": {
        const reason: TalkEndedReason = state.reason;
        this.endedAt = now;
        this.releaseUnconfirmed(now);
        if (reason === "released" || reason === "stopped") {
          // A plain release needs no message; the result of the playback check may follow.
          this.resultUntil = null;
          this.ui = { ...this.ui, phase: "idle", untilElapsedMs: null, endedReason: reason };
        } else {
          this.resultUntil = now + TALK_RESULT_VISIBLE_MS;
          this.ui = { ...this.ui, phase: "ended", untilElapsedMs: null, endedReason: reason };
        }
        break;
      }
      case "denied": {
        const denied: TalkDeniedReason = state.reason?.kind ?? "unknown";
        if (
          state.reason !== null &&
          (state.reason.kind === "cooldown" || state.reason.kind === "budget")
        ) {
          this.retryAt = now + state.reason.retry_in_ms;
        }
        this.endedAt = null;
        this.resultUntil = now + TALK_RESULT_VISIBLE_MS;
        this.ui = {
          ...this.ui,
          phase: "denied",
          untilElapsedMs: null,
          deniedReason: denied,
        };
        break;
      }
    }
  }

  private onPlayback(result: "heard" | "unconfirmed" | "unknown", now: number): void {
    if (result === "unconfirmed") {
      this.unconfirmedPending = true;
      this.unconfirmedUntil = this.busy ? null : now + TALK_UNCONFIRMED_VISIBLE_MS;
      this.ui = { ...this.ui, unconfirmed: true };
    } else if (result === "heard") {
      this.unconfirmedPending = false;
      this.unconfirmedUntil = null;
      this.ui = { ...this.ui, unconfirmed: false };
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
    if (this.busy) return null;
    if (!this.browserSupported) return "browser_unsupported";
    if (!this.connected) return "not_connected";
    const peer = this.peer;
    if (peer?.talk === "off") return "off";
    if (peer?.talk === "unsupported") return "unsupported";
    if (peer?.talk === "muted") return "muted";
    if (peer?.talker === "other") return "other_talker";
    if (this.retryAt !== null && now < this.retryAt) return "cooldown";
    return null;
  }

  private build(now: number): ParentTalkUi {
    const busy = this.busy;
    const self = busy || this.peer?.talker === "you";
    return {
      ...this.ui,
      unavailable: this.unavailable(now),
      retryAtElapsedMs: this.retryAt !== null && now < this.retryAt ? this.retryAt : null,
      noisePause: this.paused ? (self ? "self" : "other") : "none",
      audibleUncertain: this.peer?.talk_audible === "uncertain",
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
 * The baby device's talk-back view: the indicator while a parent's voice plays, the "last talk"
 * line (local only), the settings and whether the browser suspended its audio.
 */
export class BabyTalkModel {
  private ui: BabyTalkUi = defaultBabyTalk();

  constructor(
    private readonly clock: Clock,
    private readonly parentName: () => string,
    private readonly onChange: (state: BabyTalkUi) => void,
  ) {}

  get state(): BabyTalkUi {
    return this.ui;
  }

  get settings(): TalkBackSettings {
    return this.ui.settings;
  }

  /** A new run: no talk plays, the settings and the last talk of the page stay. */
  reset(): void {
    this.update({ talking: false });
  }

  onEvent(event: LinkEvent): void {
    if (event.type !== "talk_active") return;
    if (event.active) {
      this.update({ talking: true });
      return;
    }
    const endWall = this.clock.wallMs();
    this.update({
      talking: false,
      lastTalk: {
        name: this.parentName(),
        atWallMs: endWall - event.duration_ms,
        durationMs: event.duration_ms,
      },
    });
  }

  setSettings(update: Partial<TalkBackSettings>): void {
    this.update({ settings: { ...this.ui.settings, ...update } });
  }

  setSoundSuspended(suspended: boolean): void {
    this.update({ soundSuspended: suspended });
  }

  private update(patch: Partial<BabyTalkUi>): void {
    this.ui = { ...this.ui, ...patch };
    this.onChange(this.ui);
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
