import type { UnitWarning } from "@guardedsleep/core-wasm";

/**
 * Plain UI models, mirroring `apps/android/.../ui/model/UiContract.kt` in camelCase TypeScript.
 * Svelte components render only these shapes and call only the monitor controllers; they never see
 * the core's snake_case JSON. Times: `*ElapsedMs` use the link driver's monotonic clock,
 * `*WallMs` use `Date.now()` (clock times shown to the user).
 */

export type LinkPhase =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "peer_stopped"
  | "failed_needs_user"
  | "stopped";

export type CauseHint = "server_unreachable" | "peer_absent" | "media_blocked" | "update_required";

export type LinkQuality = "good" | "medium" | "poor" | "unknown";

export type LinkUi = {
  phase: LinkPhase;
  /** Escalation level 0..3 while not connected; 0 when connected. */
  level: number;
  /** Last sign of life from the peer (heartbeat, alert, status); drives "not reachable for m:ss". */
  lastAliveElapsedMs: number | null;
  /** "I'm checking" mute end (parent, level >= 2). */
  acknowledgedUntilElapsedMs: number | null;
  causeHint: CauseHint | null;
  /** Message key of a terminal failure, e.g. `pairing_key_mismatch`, `update_required`. */
  failureKey: string | null;
  quality: LinkQuality;
};

export type SilenceStatus =
  | "silent"
  | "screen_may_light"
  | "may_buzz"
  | "may_ring"
  | "unknown"
  | "no_access";

export type PeerStatusUi = {
  batteryPct: number | null;
  charging: boolean | null;
  silence: SilenceStatus;
  alarmsAllowed: boolean | null;
  nextAlarmMinutes: number | null;
  micSilent: boolean | null;
  /** The baby's noise gate threshold (RMS), for the marker on the meter. */
  noiseThreshold: number | null;
  /** Reported by a browser baby: it may pause (page hidden, no wake lock, ...). */
  unitWarnings: UnitWarning[];
  /** Baby is a browser: silence can never be read or set. */
  isWebClient: boolean;
  /** `low_battery` crossing from the baby: shown as a chip until the baby charges again. */
  lowBattery: { pct: number; critical: boolean } | null;
  chargerUnplugged: boolean;
};

export type ReadinessIssue =
  | "notifications_blocked"
  | "alarm_volume_zero"
  | "full_screen_blocked"
  | "dnd_bypass_missing"
  | "battery_optimized"
  | "parent_battery_low"
  | "headphones_connected"
  | "never_tested";

export type ListenMode = "off" | "hold" | "locked";

export type SilenceRequest = "idle" | "pending" | "applied" | "failed";

export type MissedEventsUi = {
  downMs: number;
  /** Only set when the baby's buffer was complete; otherwise the card says "we don't know". */
  count: number | null;
  complete: boolean;
  maxRms: number;
};

/** The current noise event (S10). One event covers noise bursts less than 2 minutes apart. */
export type NoiseAlertUi = {
  sinceWallMs: number;
  lastWallMs: number;
  probe: boolean;
  /** The parent pressed "Stop sound" for this event. */
  soundStopped: boolean;
};

export type ParentMonitorState = {
  link: LinkUi;
  peer: PeerStatusUi;
  /** Live level 0..1 of the baby's microphone (heartbeat RMS, scaled for the meter). */
  level: number;
  /** Start of the current quiet period; null while disconnected or unknown (never "All quiet" then). */
  quietSinceWallMs: number | null;
  /** Quiet start before the current outage; restored when the baby reports no sounds meanwhile. */
  quietSinceBeforeOutageWallMs: number | null;
  noiseAlertActive: boolean;
  alert: NoiseAlertUi | null;
  /** Last non-test noise, kept for the headline "Sound at {time}". */
  lastNoiseWallMs: number | null;
  /** The last noise arrived while snoozed (headline "alerts snoozed"). */
  lastNoiseSnoozed: boolean;
  listen: ListenMode;
  snoozedUntilWallMs: number | null;
  readiness: ReadinessIssue[];
  silenceRequest: SilenceRequest;
  missed: MissedEventsUi | null;
  /** Set for a few seconds after recovery: "Connected again after m:ss". */
  restoredAfterMs: number | null;
  securityWarning: boolean;
};

export type BabyMonitorState = {
  link: LinkUi;
  micLevel: number;
  threshold: number;
  holdSeconds: number;
  /** Always null for a browser baby: the browser cannot silence the device. */
  silenceApplied: boolean | null;
  dndAccess: boolean;
  kioskLocked: boolean;
  silenceRestoredNotice: boolean;
  micSilent: boolean;
};

export type PairingError =
  | "camera_denied"
  | "not_our_code"
  | "expired"
  | "key_mismatch"
  | "already_paired"
  | "unreachable"
  | "incomplete_paste";

export const initialLinkUi = (): LinkUi => ({
  phase: "idle",
  level: 0,
  lastAliveElapsedMs: null,
  acknowledgedUntilElapsedMs: null,
  causeHint: null,
  failureKey: null,
  quality: "unknown",
});

export const initialPeerStatus = (): PeerStatusUi => ({
  batteryPct: null,
  charging: null,
  silence: "unknown",
  alarmsAllowed: null,
  nextAlarmMinutes: null,
  micSilent: null,
  noiseThreshold: null,
  unitWarnings: [],
  isWebClient: false,
  lowBattery: null,
  chargerUnplugged: false,
});

export const initialParentState = (): ParentMonitorState => ({
  link: initialLinkUi(),
  peer: initialPeerStatus(),
  level: 0,
  quietSinceWallMs: null,
  quietSinceBeforeOutageWallMs: null,
  noiseAlertActive: false,
  alert: null,
  lastNoiseWallMs: null,
  lastNoiseSnoozed: false,
  listen: "off",
  snoozedUntilWallMs: null,
  readiness: [],
  silenceRequest: "idle",
  missed: null,
  restoredAfterMs: null,
  securityWarning: false,
});

export const initialBabyState = (): BabyMonitorState => ({
  link: initialLinkUi(),
  micLevel: 0,
  threshold: 0.035,
  holdSeconds: 5,
  silenceApplied: null,
  dndAccess: false,
  kioskLocked: false,
  silenceRestoredNotice: false,
  micSilent: false,
});
