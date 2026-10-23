import type { DeviceStatus, LinkState, LinkStatus } from "@guardedsleep/core-wasm";
import type {
  CauseHint,
  LinkPhase,
  LinkQuality,
  LinkUi,
  PeerStatusUi,
  SilenceStatus,
} from "../contract";

/*
 * Pure mappings from the core's wire values to the contract models (the web counterpart of
 * LinkMapping.kt and SilenceMapping.kt). Copy is never resolved here: a cause or failure carries
 * the core's message key (`server_unreachable`, `pairing_key_mismatch`, ...), and the UI renders
 * it with `t("core.<camelCase>")`.
 */

const PHASES: Record<LinkState, LinkPhase> = {
  connecting: "connecting",
  connected: "connected",
  reconnecting: "reconnecting",
  peer_stopped: "peer_stopped",
  failed_needs_user: "failed_needs_user",
  stopped: "stopped",
};

export function phaseOf(state: LinkState): LinkPhase {
  return PHASES[state];
}

const CAUSES: Record<string, CauseHint> = {
  server_unreachable: "server_unreachable",
  peer_absent: "peer_absent",
  media_blocked: "media_blocked",
};

export function causeHintOf(key: string | null): CauseHint | null {
  return key !== null && Object.hasOwn(CAUSES, key) ? (CAUSES[key] ?? null) : null;
}

export function linkUiOf(status: LinkStatus, quality: LinkQuality): LinkUi {
  const phase = phaseOf(status.state);
  const escalating = phase === "connecting" || phase === "reconnecting";
  return {
    phase,
    level: escalating ? status.level : 0,
    lastAliveElapsedMs: status.last_alive_ms,
    acknowledgedUntilElapsedMs: status.acknowledged_until,
    causeHint: causeHintOf(status.cause),
    // update_required also arrives as a sticky cause while connected or reconnecting.
    failureKey:
      phase === "failed_needs_user"
        ? status.key
        : status.cause === "update_required"
          ? status.cause
          : null,
    quality: phase === "connected" ? quality : "unknown",
    stalePairingHint: false,
  };
}

const MIN_PACKETS = 20;

/** Loss over the last stats interval; null when too few packets arrived to judge. */
export function qualityOf(deltaReceived: number, deltaLost: number): LinkQuality | null {
  const total = deltaReceived + deltaLost;
  if (total < MIN_PACKETS) return null;
  const loss = (Math.max(0, deltaLost) * 100) / total;
  if (loss < 3) return "good";
  if (loss < 10) return "medium";
  return "poor";
}

/** A browser baby cannot read ringer, DND or access at all, so none of them is ever reported. */
export function isWebClient(s: DeviceStatus | null): boolean {
  return s !== null && s.ringer === undefined && s.dnd === undefined && s.dnd_access === undefined;
}

export function silenceOf(s: DeviceStatus | null): SilenceStatus {
  if (s === null || isWebClient(s)) return "unknown";
  // Our own rule: alarms only, visual effects suppressed.
  if (s.quiet_active === true) return "silent";
  const dndBlocks = s.dnd === "alarms_only" || s.dnd === "total";
  let outcome: SilenceStatus;
  if (dndBlocks) outcome = "screen_may_light";
  else if (s.ringer === "vibrate") outcome = "may_buzz";
  else if (s.ringer === "normal") outcome = "may_ring";
  else if (s.ringer === "silent") outcome = "screen_may_light";
  else outcome = "unknown";
  const canMakeNoise = outcome === "may_buzz" || outcome === "may_ring";
  return canMakeNoise && s.dnd_access === false ? "no_access" : outcome;
}

export function peerStatusOf(s: DeviceStatus | null): PeerStatusUi {
  return {
    batteryPct: s?.battery_pct ?? null,
    charging: s?.charging ?? null,
    silence: silenceOf(s),
    alarmsAllowed: s?.alarms_allowed ?? null,
    nextAlarmMinutes: s?.next_alarm_min ?? null,
    micSilent: s?.mic_silent ?? null,
    isWebClient: isWebClient(s),
    unitWarnings: s?.unit_warnings ?? [],
  };
}
