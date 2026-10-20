import type { DeviceStatus, LinkEvent, LinkStatus } from "@guardedsleep/core-wasm";
import {
  initialBabyState,
  initialParentState,
  type BabyMonitorState,
  type CauseHint,
  type LinkPhase,
  type LinkUi,
  type ParentMonitorState,
  type PeerStatusUi,
  type SilenceStatus,
} from "./ui-contract.js";

/** Noise bursts closer together than this belong to one event (alpha-ux-flows §3.7). */
export const NOISE_EVENT_GAP_MS = 120_000;

/** Level meter scale: heartbeat RMS 0..~0.2 mapped onto 0..1. */
const METER_FULL_SCALE_RMS = 0.2;

export type EventContext = {
  /** Driver monotonic clock. */
  nowElapsedMs: number;
  /** `Date.now()`. */
  nowWallMs: number;
};

const CAUSES: readonly CauseHint[] = [
  "server_unreachable",
  "peer_absent",
  "media_blocked",
  "update_required",
];

export function mapLinkStatus(prev: LinkUi, status: LinkStatus, ctx: EventContext): LinkUi {
  const phase: LinkPhase = status.state;
  const cause = CAUSES.find((c) => c === status.cause) ?? null;
  return {
    ...prev,
    phase,
    level: phase === "connecting" || phase === "reconnecting" ? Math.min(3, status.level) : 0,
    causeHint: cause,
    failureKey: status.state === "failed_needs_user" ? status.key : null,
    acknowledgedUntilElapsedMs: status.acknowledged_until,
    lastAliveElapsedMs: phase === "connected" ? ctx.nowElapsedMs : status.last_alive_ms,
  };
}

/** Message key of the terminal failure for a browser that cannot expose the DTLS certificate. */
export const BROWSER_UNSUPPORTED_KEY = "browser_unsupported";

/** Sticky override: the core keeps retrying, but this unit can never complete verification. */
export function markBrowserUnsupported(link: LinkUi): LinkUi {
  return {
    ...link,
    phase: "failed_needs_user",
    level: 0,
    causeHint: null,
    failureKey: BROWSER_UNSUPPORTED_KEY,
  };
}

export function meterLevel(rms: number | null | undefined): number {
  if (rms == null || !Number.isFinite(rms) || rms <= 0) return 0;
  return Math.min(1, rms / METER_FULL_SCALE_RMS);
}

export type MeterWord = "quiet" | "some" | "loud";

export function meterWord(level: number): MeterWord {
  if (level < 0.15) return "quiet";
  if (level < 0.55) return "some";
  return "loud";
}

export function deriveSilence(s: DeviceStatus): SilenceStatus {
  const blockingDnd = s.dnd === "total" || s.dnd === "priority" || s.dnd === "alarms_only";
  if (s.quiet_active === true || blockingDnd || s.ringer === "silent") return "silent";
  if (s.ringer === undefined && s.dnd === undefined) return "unknown";
  if (s.dnd_access === false) return "no_access";
  if (s.ringer === "vibrate") return "may_buzz";
  if (s.ringer === "normal") return "may_ring";
  return "unknown";
}

/** A browser baby reports no ringer or Do Not Disturb information at all. */
export function isWebBaby(s: DeviceStatus): boolean {
  return (
    s.ringer === undefined &&
    s.dnd === undefined &&
    s.dnd_access === undefined &&
    s.alarms_allowed === undefined &&
    s.quiet_active === undefined
  );
}

export function mapPeerStatus(prev: PeerStatusUi, s: DeviceStatus): PeerStatusUi {
  const charging = s.charging ?? null;
  return {
    ...prev,
    batteryPct: s.battery_pct ?? null,
    charging,
    silence: deriveSilence(s),
    alarmsAllowed: s.alarms_allowed ?? null,
    nextAlarmMinutes: s.next_alarm_min ?? null,
    micSilent: s.mic_silent ?? null,
    noiseThreshold: s.noise_threshold ?? null,
    unitWarnings: s.unit_warnings ?? [],
    isWebClient: isWebBaby(s),
    lowBattery: charging === true ? null : prev.lowBattery,
    chargerUnplugged: charging === true ? false : prev.chargerUnplugged,
  };
}

export function reduceParentEvent(
  state: ParentMonitorState,
  ev: LinkEvent,
  ctx: EventContext,
): ParentMonitorState {
  switch (ev.type) {
    case "status": {
      const link = mapLinkStatus(state.link, ev.status, ctx);
      const connected = link.phase === "connected";
      const wasConnected = state.link.phase === "connected";
      let quietSinceWallMs = state.quietSinceWallMs;
      let quietBefore = state.quietSinceBeforeOutageWallMs;
      if (!connected) {
        if (wasConnected && quietSinceWallMs != null) quietBefore = quietSinceWallMs;
        quietSinceWallMs = null;
      } else if (quietSinceWallMs == null && state.alert == null) {
        quietSinceWallMs = ctx.nowWallMs;
      }
      return { ...state, link, quietSinceWallMs, quietSinceBeforeOutageWallMs: quietBefore };
    }
    case "peer_heartbeat":
      return {
        ...state,
        link: { ...state.link, lastAliveElapsedMs: ctx.nowElapsedMs },
        level: ev.rms == null ? state.level : meterLevel(ev.rms),
      };
    case "peer_status":
      return {
        ...state,
        link: { ...state.link, lastAliveElapsedMs: ctx.nowElapsedMs },
        peer: mapPeerStatus(state.peer, ev.status),
      };
    case "noise_alert": {
      const link = { ...state.link, lastAliveElapsedMs: ctx.nowElapsedMs };
      if (ev.snoozed && !ev.probe) {
        return { ...state, link, lastNoiseWallMs: ctx.nowWallMs, lastNoiseSnoozed: true };
      }
      const prev = state.alert;
      const continues =
        prev != null &&
        prev.probe === ev.probe &&
        ctx.nowWallMs - prev.lastWallMs < NOISE_EVENT_GAP_MS;
      return {
        ...state,
        link,
        noiseAlertActive: true,
        alert: {
          sinceWallMs: continues ? prev.sinceWallMs : ctx.nowWallMs,
          lastWallMs: ctx.nowWallMs,
          probe: ev.probe,
          soundStopped: continues ? prev.soundStopped : false,
        },
        quietSinceWallMs: null,
        ...(ev.probe ? {} : { lastNoiseWallMs: ctx.nowWallMs, lastNoiseSnoozed: false }),
      };
    }
    case "alarm_acknowledged":
      return { ...state, link: { ...state.link, acknowledgedUntilElapsedMs: ev.until_ms } };
    case "link_restored":
      return { ...state, restoredAfterMs: ev.down_ms };
    case "security_warning":
      return { ...state, securityWarning: true };
    case "low_battery":
      return {
        ...state,
        peer: { ...state.peer, lowBattery: { pct: ev.pct, critical: ev.critical } },
      };
    case "charger_unplugged":
      return { ...state, peer: { ...state.peer, chargerUnplugged: true } };
    case "missed_events": {
      const covered = ev.complete && ev.count === 0;
      return {
        ...state,
        missed: {
          downMs: ev.down_ms,
          count: ev.complete ? ev.count : null,
          complete: ev.complete,
          maxRms: ev.max_rms,
        },
        quietSinceWallMs:
          covered && state.quietSinceWallMs != null && state.quietSinceBeforeOutageWallMs != null
            ? state.quietSinceBeforeOutageWallMs
            : state.quietSinceWallMs,
      };
    }
    case "listen_changed":
      return { ...state, listen: ev.enabled ? state.listen : "off" };
    default:
      return state;
  }
}

/** Time-driven transitions: an event ends after 2 quiet minutes; `restoredAfterMs` is cleared by the controller. */
export function tickParentState(state: ParentMonitorState, ctx: EventContext): ParentMonitorState {
  const alert = state.alert;
  let next = state;
  if (alert && ctx.nowWallMs - alert.lastWallMs >= NOISE_EVENT_GAP_MS) {
    next = {
      ...next,
      alert: null,
      noiseAlertActive: false,
      quietSinceWallMs: state.link.phase === "connected" ? alert.lastWallMs : null,
    };
  }
  if (next.snoozedUntilWallMs != null && ctx.nowWallMs >= next.snoozedUntilWallMs) {
    next = { ...next, snoozedUntilWallMs: null };
  }
  return next;
}

export function reduceBabyEvent(
  state: BabyMonitorState,
  ev: LinkEvent,
  ctx: EventContext,
): BabyMonitorState {
  if (ev.type === "status") {
    return { ...state, link: mapLinkStatus(state.link, ev.status, ctx) };
  }
  if (ev.type === "peer_heartbeat" || ev.type === "peer_status") {
    return { ...state, link: { ...state.link, lastAliveElapsedMs: ctx.nowElapsedMs } };
  }
  return state;
}

export { initialBabyState, initialParentState };
