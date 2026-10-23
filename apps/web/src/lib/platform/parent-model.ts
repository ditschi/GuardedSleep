import type { DeviceStatus, LinkEvent, LinkStatus } from "@guardedsleep/core-wasm";
import {
  defaultLinkUi,
  defaultParentMonitor,
  type LinkPhase,
  type LinkQuality,
  type ListenMode,
  type MissedEventsUi,
  type ParentMonitorState,
  type ReadinessIssue,
  type SilenceRequest,
} from "../contract";
import type { AlertSink, LinkSnapshot } from "./alert-sink";
import type { Clock } from "./clock";
import type { InboundAudioStats } from "./media-engine";
import { NoiseTracker } from "./noise-tracker";
import { LISTEN_MAX_MS, MISSED_EVENTS_GRACE_MS, RESTORED_NOTICE_MS } from "./policy";
import { causeHintOf, linkUiOf, peerStatusOf, phaseOf, qualityOf } from "./status-mapping";

/** Side effects of the parent model that are not alerts. */
export interface ParentEffects {
  /** Playout of the received baby audio ("Stop sound", snoozed noise). */
  muteRemote(muted: boolean): void;
  /** Listen was switched off by the model (60 min limit, outage reset). */
  listenStopped(): void;
  /** A probe event arrived: the user's alert chain has just been exercised. */
  testAlertArrived(wallMs: number): void;
  snoozeEnded(): void;
}

const DECAY = 0.85;
const STALE_PAIRING_MS = 10 * 60_000;
const STALE_AUDIO_MS = 400;
const HEARTBEAT_VISIBLE_MS = 12_000;
const SILENCE_TIMEOUT_MS = 10_000;
const APPLIED_VISIBLE_MS = 6_000;
const LOUD_RECENT_MS = 3_000;

/**
 * Maps the core's status and events onto `ParentMonitorState` (ADR-017 section 7,
 * alpha-ux-flows section 3), and tells the alert sink what to sound. It decides nothing about the
 * link: the phase, the level, the cause and the alert events come from the core; this only turns
 * them into what the screen shows (quiet since, noise event, restored notice, missed summary).
 * Every change is pushed to `onChange`.
 */
export class ParentModel {
  private phase: LinkPhase = "idle";
  private last: LinkStatus | null = null;
  private quality: LinkQuality = "unknown";
  private statsPrev: InboundAudioStats | null = null;

  private quietSinceWall: number | null = null;
  private quietBeforeOutageWall: number | null = null;
  private reconnectWall: number | null = null;
  private awaitingMissedUntil: number | null = null;
  private restoredAfterMs: number | null = null;
  private restoredUntil: number | null = null;

  private readonly tracker = new NoiseTracker();
  private noiseSinceWall: number | null = null;
  private noiseLastWall: number | null = null;
  private noiseIsTest = false;
  private remoteLevel = 0;
  private remoteAt = Number.NEGATIVE_INFINITY;
  private heartbeatRms = 0;
  private heartbeatAt = Number.NEGATIVE_INFINITY;
  private lastLoudLevelAt = Number.NEGATIVE_INFINITY;

  private listen: ListenMode = "off";
  private listenSince = 0;
  private snoozeUntilWall: number | null = null;
  private snoozeUntilElapsed: number | null = null;

  private peerStatus: DeviceStatus | null = null;
  private silenceRequest: SilenceRequest = "idle";
  private silenceRequestedAt = 0;
  private silenceDoneAt = 0;

  private missed: MissedEventsUi | null = null;
  private security = false;
  private readiness: ReadinessIssue[] = [];
  private ownBatteryPct: number | null = null;
  private lastSnapshot: LinkSnapshot | null = null;
  private everConnected = false;
  private peerAbsentSince: number | null = null;

  constructor(
    private readonly clock: Clock,
    private readonly sink: AlertSink,
    private readonly effects: ParentEffects,
    private readonly threshold: () => number,
    private readonly onChange: (state: ParentMonitorState) => void,
  ) {}

  /** Forget the run (keeps readiness and an active snooze, which belong to the device). */
  reset(): void {
    this.phase = "idle";
    this.last = null;
    this.quality = "unknown";
    this.statsPrev = null;
    this.quietSinceWall = null;
    this.quietBeforeOutageWall = null;
    this.reconnectWall = null;
    this.everConnected = false;
    this.peerAbsentSince = null;
    this.awaitingMissedUntil = null;
    this.restoredAfterMs = null;
    this.restoredUntil = null;
    this.tracker.reset();
    this.noiseSinceWall = null;
    this.noiseLastWall = null;
    this.noiseIsTest = false;
    this.remoteLevel = 0;
    this.heartbeatRms = 0;
    this.heartbeatAt = Number.NEGATIVE_INFINITY;
    this.listen = "off";
    this.peerStatus = null;
    this.silenceRequest = "idle";
    this.missed = null;
    this.security = false;
    this.lastSnapshot = null;
    this.publish(this.clock.elapsedMs());
  }

  setReadiness(issues: ReadinessIssue[], ownBatteryPct: number | null = null): void {
    this.readiness = issues;
    this.ownBatteryPct = ownBatteryPct;
    this.publish(this.clock.elapsedMs());
  }

  /** Monotonic end of the active snooze, for `set_snooze`. */
  snoozeUntilElapsedMs(): number | null {
    return this.snoozeUntilElapsed;
  }

  snooze(minutes: number): void {
    const now = this.clock.elapsedMs();
    this.snoozeUntilElapsed = now + minutes * 60_000;
    this.snoozeUntilWall = this.clock.wallMs() + minutes * 60_000;
    this.publish(now);
  }

  cancelSnooze(): void {
    this.snoozeUntilElapsed = null;
    this.snoozeUntilWall = null;
    this.publish(this.clock.elapsedMs());
  }

  get state(): ParentMonitorState {
    return this.build(this.clock.elapsedMs());
  }

  // ---- link --------------------------------------------------------------------------------

  onStatus(status: LinkStatus, now: number): void {
    const prev = this.phase;
    const next = phaseOf(status.state);
    this.last = status;
    this.phase = next;
    if (next === "connected") this.everConnected = true;
    this.peerAbsentSince = status.cause === "peer_absent" ? (this.peerAbsentSince ?? now) : null;
    const wall = this.clock.wallMs();
    if (next === "connected" && prev !== "connected") {
      this.statsPrev = null;
      this.reconnectWall = wall;
      if (prev === "reconnecting") {
        // After an outage "quiet" must be earned: only a complete summary restores it.
        this.quietSinceWall = null;
        this.awaitingMissedUntil = now + MISSED_EVENTS_GRACE_MS;
      } else {
        this.quietSinceWall = wall;
        this.quietBeforeOutageWall = null;
        this.awaitingMissedUntil = null;
      }
    } else if (next !== "connected" && prev === "connected") {
      this.quietBeforeOutageWall = this.quietSinceWall;
      this.quietSinceWall = null;
      this.awaitingMissedUntil = null;
      this.quality = "unknown";
    }
    if (next !== "connected") this.quietSinceWall = null;
    if (next === "stopped") this.remoteLevel = 0;
    this.publishLink(now);
    this.publish(now);
  }

  onEvent(event: LinkEvent, now: number): void {
    switch (event.type) {
      case "link_lost":
        this.sink.linkLost(event.down_ms);
        break;
      case "link_restored":
        this.restoredAfterMs = event.down_ms;
        this.restoredUntil = now + RESTORED_NOTICE_MS;
        this.sink.notice({ type: "link_restored", downMs: event.down_ms });
        break;
      case "security_warning":
        this.security = true;
        this.sink.notice({ type: "security_warning" });
        break;
      case "peer_stopped":
        this.sink.notice({ type: "peer_stopped" });
        break;
      case "alarm_acknowledged":
        this.publishLink(now);
        break;
      case "noise_alert":
        this.onNoise(event.rms, event.probe, event.snoozed, now);
        break;
      case "peer_heartbeat":
        this.heartbeatRms = event.rms ?? 0;
        this.heartbeatAt = now;
        break;
      case "peer_status":
        this.onPeerStatus(event.status, now);
        break;
      case "low_battery":
        this.sink.notice({ type: "low_battery", pct: event.pct, critical: event.critical });
        break;
      case "charger_unplugged":
        this.sink.notice({ type: "charger_unplugged" });
        break;
      case "missed_events":
        this.onMissed(event.count, event.max_rms, event.complete, event.down_ms, now);
        break;
      case "listen_changed":
        if (!event.enabled && this.listen !== "off") {
          this.listen = "off";
          this.effects.listenStopped();
        }
        break;
      // Baby-side gate echo; the parent learns the real threshold from peer_status.
      case "noise_gate_changed":
      case "status":
      case "failed":
        break;
    }
    this.publish(now);
  }

  onRemoteLevel(rms: number, now: number): void {
    this.remoteLevel = Math.max(rms, this.remoteLevel * DECAY);
    this.remoteAt = now;
    const threshold = this.threshold();
    this.tracker.onLevel(now, rms, threshold);
    if (rms >= threshold) {
      this.lastLoudLevelAt = now;
      if (this.tracker.active) this.noiseLastWall = this.clock.wallMs();
    }
    this.publish(now);
  }

  onInboundStats(stats: InboundAudioStats, now: number): void {
    const prev = this.statsPrev;
    this.statsPrev = stats;
    if (prev) {
      const q = qualityOf(
        stats.packetsReceived - prev.packetsReceived,
        stats.packetsLost - prev.packetsLost,
      );
      if (q) this.quality = q;
    }
    this.publishLink(now);
    this.publish(now);
  }

  onTick(now: number): void {
    if (now - this.remoteAt > STALE_AUDIO_MS) this.remoteLevel *= DECAY;
    if (this.tracker.tick(now)) {
      const wall = this.clock.wallMs();
      this.quietSinceWall =
        this.phase === "connected" ? wall - (now - this.tracker.lastLoudAt) : null;
      this.sink.noiseEnded();
    }
    if (this.restoredUntil !== null && now >= this.restoredUntil) {
      this.restoredUntil = null;
      this.restoredAfterMs = null;
    }
    if (this.awaitingMissedUntil !== null && now >= this.awaitingMissedUntil) {
      // No summary arrived (baby restarted, or older peer): the honest fallback is the
      // reconnect time.
      this.awaitingMissedUntil = null;
      this.quietSinceWall = this.reconnectWall;
    }
    if (this.listen === "locked" && now - this.listenSince >= LISTEN_MAX_MS) {
      this.listen = "off";
      this.effects.listenStopped();
    }
    if (this.silenceRequest === "pending" && now - this.silenceRequestedAt >= SILENCE_TIMEOUT_MS) {
      this.silenceRequest = "failed";
      this.silenceDoneAt = now;
    }
    if (this.silenceRequest === "applied" && now - this.silenceDoneAt >= APPLIED_VISIBLE_MS) {
      this.silenceRequest = "idle";
    }
    if (this.snoozeUntilElapsed !== null && now >= this.snoozeUntilElapsed) {
      this.snoozeUntilElapsed = null;
      this.snoozeUntilWall = null;
      this.effects.snoozeEnded();
      // Noise that is still going at the end of the snooze alerts immediately.
      if (this.tracker.active && now - this.lastLoudLevelAt < LOUD_RECENT_MS) {
        this.effects.muteRemote(false);
        this.sink.noiseAlert("new", false, false);
      }
    }
    this.publish(now);
  }

  // ---- user intents ------------------------------------------------------------------------

  /** False when the request is ignored (not connected). */
  setListen(mode: ListenMode, now: number): boolean {
    if (this.phase !== "connected" && mode !== "off") return false;
    if (mode === "locked" && this.listen !== "locked") this.listenSince = now;
    this.listen = mode;
    if (mode !== "off") this.effects.muteRemote(false);
    this.publish(now);
    return true;
  }

  stopAlertSound(): void {
    this.effects.muteRemote(true);
    this.sink.stopNoiseSound();
  }

  requestSilence(now: number): void {
    this.silenceRequest = "pending";
    this.silenceRequestedAt = now;
    this.publish(now);
  }

  dismissMissed(): void {
    this.missed = null;
    this.publish(this.clock.elapsedMs());
  }

  // ---- internals ---------------------------------------------------------------------------

  private onNoise(rms: number, probe: boolean, snoozed: boolean, now: number): void {
    void rms;
    const wall = this.clock.wallMs();
    const kind = this.tracker.onAlert(now);
    this.lastLoudLevelAt = now;
    if (kind === "new" || this.noiseSinceWall === null) {
      this.noiseSinceWall = wall;
      this.noiseIsTest = probe;
    }
    this.noiseLastWall = wall;
    this.quietSinceWall = null;
    if (probe) this.effects.testAlertArrived(wall);
    // Snoozed noise stays visual: the baby still streams, but nothing plays.
    this.effects.muteRemote(snoozed && this.listen === "off");
    this.sink.noiseAlert(kind, probe, snoozed);
  }

  private onPeerStatus(s: DeviceStatus, now: number): void {
    this.peerStatus = s;
    if (this.silenceRequest === "pending" && s.quiet_active === true) {
      this.silenceRequest = "applied";
      this.silenceDoneAt = now;
    }
  }

  private onMissed(
    count: number,
    maxRms: number,
    complete: boolean,
    downMs: number,
    now: number,
  ): void {
    void now;
    this.missed = {
      downMs,
      count: complete ? count : null,
      complete,
      endWallMs: null,
      loudest: complete ? maxRms : null,
    };
    if (this.awaitingMissedUntil !== null) {
      this.awaitingMissedUntil = null;
      const covered = complete && count === 0;
      this.quietSinceWall = covered
        ? (this.quietBeforeOutageWall ?? this.reconnectWall)
        : this.reconnectWall;
    }
    this.sink.notice({ type: "missed_events", count: complete ? count : null, downMs });
  }

  private publishLink(now: number): void {
    const s = this.last;
    if (!s) return;
    const ui = linkUiOf(s, this.quality);
    const snapshot: LinkSnapshot = {
      phase: this.phase,
      level: ui.level,
      acknowledged: s.acknowledged_until !== null && now < s.acknowledged_until,
      causeHint: causeHintOf(s.cause),
      wasConnected: this.everConnected,
    };
    const prev = this.lastSnapshot;
    if (
      !prev ||
      prev.phase !== snapshot.phase ||
      prev.level !== snapshot.level ||
      prev.acknowledged !== snapshot.acknowledged ||
      prev.causeHint !== snapshot.causeHint ||
      prev.wasConnected !== snapshot.wasConnected
    ) {
      this.lastSnapshot = snapshot;
      this.sink.linkState(snapshot);
    }
  }

  private staleHint(now: number): boolean {
    return this.peerAbsentSince !== null && now - this.peerAbsentSince > STALE_PAIRING_MS;
  }

  private meter(now: number): number {
    const hb = now - this.heartbeatAt < HEARTBEAT_VISIBLE_MS ? this.heartbeatRms : 0;
    return Math.min(1, Math.max(0, Math.max(this.remoteLevel, hb)));
  }

  private publish(now: number): void {
    this.onChange(this.build(now));
  }

  private build(now: number): ParentMonitorState {
    const connected = this.phase === "connected";
    const base = defaultParentMonitor();
    const s = this.last;
    const noiseSince = this.noiseSinceWall;
    return {
      ...base,
      link: s
        ? { ...linkUiOf(s, this.quality), stalePairingHint: this.staleHint(now) }
        : defaultLinkUi(),
      peer: peerStatusOf(this.peerStatus),
      babyThreshold: this.peerStatus?.noise_threshold ?? null,
      level: connected ? this.meter(now) : 0,
      quietSinceWallMs: this.quietSinceWall,
      noiseAlertActive: this.tracker.active,
      listen: this.listen,
      snoozedUntilWallMs: this.snoozeUntilWall,
      readiness: this.readiness,
      silenceRequest: this.silenceRequest,
      missed: this.missed,
      restoredAfterMs: this.restoredAfterMs,
      securityWarning: this.security,
      noiseSinceWallMs: this.tracker.active ? noiseSince : null,
      lastSound:
        noiseSince === null
          ? null
          : {
              atWallMs: noiseSince,
              durationMs: Math.max(0, (this.noiseLastWall ?? noiseSince) - noiseSince),
            },
      ownBatteryPct: this.ownBatteryPct,
      noiseIsTest: this.noiseIsTest && this.tracker.active,
    };
  }
}
