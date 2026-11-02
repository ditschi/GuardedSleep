import type { ParentMonitorState, LinkUi, SilenceStatus } from "../contract";
import type { MessageKey } from "../i18n/keys";
import type { GlyphName, Tone } from "./glyphs";

/*
 * What the parent screens show, derived from the contract model: the ambient bar, the headline,
 * the chip row and which full-screen takes over (alpha-ux-flows.md sections 4.8 and 5). Pure
 * functions of the state and the two clocks, so every case is a table test. Nothing here decides
 * about the link: phase, level and alerts come from the core through the model.
 */

// ---- ambient bar -----------------------------------------------------------------------------

export type AmbientKind =
  | "connected"
  | "connecting"
  | "reconnecting"
  | "cant_reach"
  | "lost_for"
  | "baby_stopped"
  | "needs_pairing"
  | "idle";

export interface AmbientLook {
  tone: Tone;
  icon: GlyphName;
  /** Escalation level 2 and above is announced assertively (alpha-ux-flows.md section 6.1). */
  assertive: boolean;
}

export function ambientKind(link: LinkUi): AmbientKind {
  switch (link.phase) {
    case "connected":
      return "connected";
    case "connecting":
    case "reconnecting":
      if (link.level >= 3) return "lost_for";
      if (link.level >= 2) return "cant_reach";
      return link.phase === "connecting" && link.level === 0 ? "connecting" : "reconnecting";
    case "peer_stopped":
      return "baby_stopped";
    case "failed_needs_user":
      return "needs_pairing";
    case "idle":
    case "stopped":
      return "idle";
  }
}

export const AMBIENT_LOOK: Readonly<Record<AmbientKind, AmbientLook>> = {
  connected: { tone: "ok", icon: "link", assertive: false },
  connecting: { tone: "warn", icon: "refresh", assertive: false },
  reconnecting: { tone: "warn", icon: "refresh", assertive: false },
  cant_reach: { tone: "orange", icon: "linkOff", assertive: true },
  lost_for: { tone: "bad", icon: "alert", assertive: true },
  baby_stopped: { tone: "neutral", icon: "pause", assertive: false },
  needs_pairing: { tone: "bad", icon: "alert", assertive: true },
  idle: { tone: "neutral", icon: "pause", assertive: false },
};

export const AMBIENT_KEY: Readonly<Record<AmbientKind, MessageKey>> = {
  connected: "native.ui.link.connected",
  connecting: "native.ui.link.connecting",
  reconnecting: "native.ui.link.reconnecting",
  cant_reach: "native.ui.link.cantReach",
  lost_for: "native.ui.link.lostFor",
  baby_stopped: "native.ui.link.babyStopped",
  needs_pairing: "core.pairingKeyMismatch",
  idle: "native.ui.link.idle",
};

/** How long the link has been down, from the last contact; 0 when nothing was ever heard. */
export function downForMs(link: LinkUi, nowElapsedMs: number): number {
  return link.lastAliveElapsedMs === null ? 0 : Math.max(0, nowElapsedMs - link.lastAliveElapsedMs);
}

// ---- headline --------------------------------------------------------------------------------

export type Headline =
  | { kind: "all_quiet"; sinceWallMs: number }
  | { kind: "sound_since"; sinceWallMs: number }
  | { kind: "sound_at"; atWallMs: number; durationMs: number }
  | { kind: "sound_snoozed"; atWallMs: number }
  | { kind: "last_heard"; atWallMs: number }
  | { kind: "no_contact" }
  | { kind: "monitoring" };

/** Never "All quiet" while the link is not connected (alpha-ux-flows.md section 5, row 2). */
export function headlineOf(
  p: ParentMonitorState,
  nowElapsedMs: number,
  nowWallMs: number,
): Headline {
  if (p.link.phase !== "connected") {
    if (p.link.lastAliveElapsedMs === null) return { kind: "no_contact" };
    return {
      kind: "last_heard",
      atWallMs: nowWallMs - downForMs(p.link, nowElapsedMs),
    };
  }
  if (p.noiseAlertActive && p.noiseSinceWallMs !== null) {
    return { kind: "sound_since", sinceWallMs: p.noiseSinceWallMs };
  }
  if (p.quietSinceWallMs !== null) return { kind: "all_quiet", sinceWallMs: p.quietSinceWallMs };
  if (p.lastSound !== null) {
    const snoozed = p.snoozedUntilWallMs !== null && p.snoozedUntilWallMs > nowWallMs;
    return snoozed
      ? { kind: "sound_snoozed", atWallMs: p.lastSound.atWallMs }
      : { kind: "sound_at", atWallMs: p.lastSound.atWallMs, durationMs: p.lastSound.durationMs };
  }
  return { kind: "monitoring" };
}

// ---- full screen -----------------------------------------------------------------------------

export type FullScreenKind =
  "link_lost" | "noise" | "needs_pairing" | "baby_stopped" | "battery_critical";

/** Below this the baby device is about to switch off (alpha-ux-flows.md section 4.10). */
export const BATTERY_CRITICAL_PCT = 5;

/**
 * Only one full screen at a time: connection lost, then noise alert, then the baby device
 * stopped, then its battery (section 5). "I'm checking" hides the lost-connection screen for the
 * mute period; the ambient bar keeps showing the state. `dismissed` holds the kinds the user
 * closed for the current situation ("Stop sound" closes the noise alert, OK the others).
 */
export function fullScreenOf(
  p: ParentMonitorState,
  nowElapsedMs: number,
  dismissed: ReadonlySet<FullScreenKind>,
): FullScreenKind | null {
  const link = p.link;
  const escalating = link.phase === "connecting" || link.phase === "reconnecting";
  const acknowledged =
    link.acknowledgedUntilElapsedMs !== null && link.acknowledgedUntilElapsedMs > nowElapsedMs;
  if (escalating && link.level >= 2 && !acknowledged) return "link_lost";
  if (p.noiseAlertActive && !dismissed.has("noise")) return "noise";
  if (link.phase === "failed_needs_user") return "needs_pairing";
  if (link.phase === "peer_stopped" && !dismissed.has("baby_stopped")) return "baby_stopped";
  if (batteryCritical(p) && !dismissed.has("battery_critical")) return "battery_critical";
  return null;
}

/** The baby device is about to switch off and is not charging. */
export function batteryCritical(p: ParentMonitorState): boolean {
  const pct = p.peer.batteryPct;
  return (
    p.link.phase === "connected" &&
    pct !== null &&
    pct < BATTERY_CRITICAL_PCT &&
    p.peer.charging !== true
  );
}

/**
 * Drops the dismissals whose situation is over, so the next noise event, stop or low battery
 * shows its full screen again. Returns the same set when nothing changed.
 */
export function pruneDismissed(
  p: ParentMonitorState,
  dismissed: ReadonlySet<FullScreenKind>,
): ReadonlySet<FullScreenKind> {
  const next = new Set(dismissed);
  if (!p.noiseAlertActive) next.delete("noise");
  if (p.link.phase !== "peer_stopped") next.delete("baby_stopped");
  if (!batteryCritical(p)) next.delete("battery_critical");
  return next.size === dismissed.size ? dismissed : next;
}

// ---- chips -----------------------------------------------------------------------------------

export interface ChipSpec {
  /** The stable test id. */
  id: string;
  tone: Tone;
  icon: GlyphName;
  key: MessageKey;
  args?: Record<string, number>;
}

/** The battery level below which the baby device asks to be plugged in. */
export const BATTERY_LOW_PCT = 20;

const SILENCE_CHIP: Readonly<Record<SilenceStatus, { key: MessageKey; tone: Tone }>> = {
  silent: { key: "native.ui.silence.silent", tone: "ok" },
  screen_may_light: { key: "native.ui.silence.screenMayLight", tone: "warn" },
  may_buzz: { key: "native.ui.silence.mayVibrate", tone: "warn" },
  may_ring: { key: "native.ui.silence.mayRing", tone: "warn" },
  unknown: { key: "native.ui.silence.unknown", tone: "neutral" },
  no_access: { key: "native.ui.silence.noAccess", tone: "warn" },
};

const UNIT_WARNING_KEYS: Readonly<Record<string, MessageKey>> = {
  page_hidden: "native.ui.parent.unitWarning.pageHidden",
  wake_lock_off: "native.ui.parent.unitWarning.wakeLockOff",
  resumed_from_sleep: "native.ui.parent.unitWarning.resumedFromSleep",
  not_charging: "native.ui.parent.unitWarning.notCharging",
  mic_permission_lost: "native.ui.parent.unitWarning.micPermissionLost",
};

/** Battery and silence of the baby device: always shown, neutral unless something is wrong. */
export function fixedChips(p: ParentMonitorState): ChipSpec[] {
  const chips: ChipSpec[] = [];
  const pct = p.peer.batteryPct;
  if (pct !== null) {
    const low = pct < BATTERY_LOW_PCT && p.peer.charging !== true;
    chips.push({
      id: "parent_baby_battery_chip",
      tone: low ? "warn" : "neutral",
      icon: "battery",
      key: low ? "native.ui.battery.babyLow" : "native.ui.battery.baby",
      args: { percent: pct },
    });
  }
  if (p.peer.isWebClient) {
    // A browser baby device can neither report nor change its silence: grey, with the manual hint.
    chips.push({
      id: "parent_silence_chip",
      tone: "neutral",
      icon: "info",
      key: "webScreens.parent.quietUnknownWeb",
    });
  } else {
    const look = SILENCE_CHIP[p.peer.silence];
    chips.push({ id: "parent_silence_chip", tone: look.tone, icon: "info", key: look.key });
  }
  return chips;
}

/** Warnings, in the priority of section 5: the screen shows the first two and "+N". */
export function extraChips(p: ParentMonitorState, nowWallMs: number): ChipSpec[] {
  const chips: ChipSpec[] = [];
  if (p.ownBatteryPct !== null && p.ownBatteryPct < BATTERY_LOW_PCT) {
    chips.push({
      id: "parent_own_battery_chip",
      tone: "warn",
      icon: "battery",
      key: "native.ui.battery.parentLow",
      args: { percent: p.ownBatteryPct },
    });
  }
  for (const warning of p.peer.unitWarnings) {
    const key = UNIT_WARNING_KEYS[warning];
    if (key) {
      chips.push({ id: `parent_unit_warning_${warning}`, tone: "warn", icon: "alert", key });
    }
  }
  if (p.snoozedUntilWallMs !== null && p.snoozedUntilWallMs > nowWallMs) {
    chips.push({
      id: "parent_snooze_chip",
      tone: "snooze",
      icon: "snooze",
      key: "native.ui.snooze.active",
      args: { minutes: Math.max(1, Math.ceil((p.snoozedUntilWallMs - nowWallMs) / 60_000)) },
    });
  }
  if (p.link.phase === "connected" && p.link.quality === "poor") {
    chips.push({
      id: "parent_quality_chip",
      tone: "warn",
      icon: "alert",
      key: "native.ui.link.weak",
    });
  }
  return chips;
}

/** Whether the baby device can be asked to silence itself (a browser baby device cannot). */
export function canRequestSilence(p: ParentMonitorState): boolean {
  if (p.peer.isWebClient || p.link.phase !== "connected") return false;
  return (
    p.peer.silence === "may_ring" ||
    p.peer.silence === "may_buzz" ||
    p.peer.silence === "screen_may_light"
  );
}
