import type {
  LinkUi,
  MissedEventsUi,
  ParentMonitorState,
  PeerStatusUi,
  SilenceStatus,
} from "../contract";
import type { MessageKey } from "../i18n/keys";
import type { Translate } from "../i18n/translate";
import type { GlyphName, Tone } from "./glyphs";
import { coreMessageKey } from "./messages";

/*
 * What the parent screens show, derived from the contract model: the ambient bar, the headline,
 * the chip row and which full-screen takes over (alpha-ux-flows.md sections 4.8 and 5). Pure
 * functions of the state and the two clocks, so every case is a table test. Nothing here decides
 * about the link: phase, level and alerts come from the core through the model.
 */

// ---- status bar ------------------------------------------------------------------------------

/**
 * The stage of the link as the bar words it (`connection-status` "Ambient bar per level"). Colour
 * and words follow the core's state and level only; there is no threshold or timer here
 * (AGENTS.md section 5.1). The Android client mirrors this table in `StatusModel.kt`.
 */
export type StatusKind =
  | "connected"
  | "connecting"
  | "reconnecting"
  | "not_connected_yet"
  | "cant_reach"
  | "couldnt_connect"
  | "no_connection"
  | "still_no_connection"
  | "baby_stopped"
  | "needs_pairing"
  | "idle";

export interface StatusLook {
  tone: Tone;
  icon: GlyphName;
  /** Stage changes from level 2 are announced assertively (`connection-status` announcements). */
  assertive: boolean;
  /** Red with the outage duration beside the words. */
  loud: boolean;
}

export function statusKindOf(link: LinkUi): StatusKind {
  switch (link.phase) {
    case "connected":
      return "connected";
    case "connecting":
    case "reconnecting": {
      const first = link.phase === "connecting";
      if (link.level >= 3) return "still_no_connection";
      if (link.level === 2) return first ? "couldnt_connect" : "no_connection";
      if (link.level === 1) return first ? "not_connected_yet" : "cant_reach";
      return first ? "connecting" : "reconnecting";
    }
    case "peer_stopped":
      return "baby_stopped";
    case "failed_needs_user":
      return "needs_pairing";
    case "idle":
    case "stopped":
      return "idle";
  }
}

export const STATUS_LOOK: Readonly<Record<StatusKind, StatusLook>> = {
  connected: { tone: "ok", icon: "link", assertive: false, loud: false },
  connecting: { tone: "info", icon: "refresh", assertive: false, loud: false },
  reconnecting: { tone: "info", icon: "refresh", assertive: false, loud: false },
  not_connected_yet: { tone: "orange", icon: "linkOff", assertive: false, loud: false },
  cant_reach: { tone: "orange", icon: "linkOff", assertive: false, loud: false },
  couldnt_connect: { tone: "bad", icon: "linkOff", assertive: true, loud: true },
  no_connection: { tone: "bad", icon: "linkOff", assertive: true, loud: true },
  still_no_connection: { tone: "bad", icon: "linkOff", assertive: true, loud: true },
  baby_stopped: { tone: "neutral", icon: "pause", assertive: true, loud: false },
  needs_pairing: { tone: "bad", icon: "alert", assertive: true, loud: false },
  idle: { tone: "neutral", icon: "pause", assertive: false, loud: false },
};

/** The state in words; "State · Name" is composed by the bar, never spliced into a sentence. */
export const STATUS_KEY: Readonly<Record<StatusKind, MessageKey>> = {
  connected: "native.ui.status.connected",
  connecting: "native.ui.status.connecting",
  reconnecting: "native.ui.status.reconnecting",
  not_connected_yet: "native.ui.status.notConnectedYet",
  cant_reach: "native.ui.status.cantReach",
  couldnt_connect: "native.ui.status.couldntConnect",
  no_connection: "native.ui.status.noConnection",
  still_no_connection: "native.ui.status.stillNoConnection",
  baby_stopped: "native.ui.status.stopped",
  needs_pairing: "core.pairingKeyMismatch",
  idle: "native.ui.link.idle",
};

/** The connection's own name, or null when it has none (blank counts as none). */
export function connectionName(peerName: string | null): string | null {
  return peerName?.trim() || null;
}

/**
 * The stage in words, "State · Name" when the connection has a name; the name is never spliced into
 * a sentence. A link that needs the user says the core's own message, an idle bar carries no name.
 */
export function statusHeadline(
  t: Translate,
  kind: StatusKind,
  link: LinkUi,
  name: string | null,
): string {
  if (kind === "needs_pairing")
    return t(coreMessageKey(link.failureKey) ?? STATUS_KEY.needs_pairing);
  const state = t(STATUS_KEY[kind]);
  return name === null || kind === "idle" ? state : t("native.ui.status.withName", { state, name });
}

/** The one sub-line a stage has, if any. */
export const STATUS_SUB_KEY: Readonly<Partial<Record<StatusKind, MessageKey>>> = {
  connecting: "native.ui.status.subConnecting",
  reconnecting: "native.ui.status.subReconnecting",
  not_connected_yet: "native.ui.status.subNotAnswered",
  cant_reach: "native.ui.status.subStillTrying",
  baby_stopped: "native.ui.status.subStopped",
};

/** The outage start on the core clock; the last contact when the core sent no start. */
export function outageStartElapsedMs(link: LinkUi): number | null {
  return link.downSinceElapsedMs ?? link.lastAliveElapsedMs;
}

/** How long the link has been down; 0 when nothing is known. */
export function downForMs(link: LinkUi, nowElapsedMs: number): number {
  const start = outageStartElapsedMs(link);
  return start === null ? 0 : Math.max(0, nowElapsedMs - start);
}

/** The three stages of the full-screen connection alert. */
export type AlertStage = "vibrating" | "loud" | "paused";

export function alertStageOf(link: LinkUi, nowElapsedMs: number): AlertStage {
  if (link.acknowledgedUntilElapsedMs !== null && link.acknowledgedUntilElapsedMs > nowElapsedMs) {
    return "paused";
  }
  return link.level >= 3 ? "loud" : "vibrating";
}

/**
 * A core-clock time as a wall-clock time, for "Loud alarm at 03:17". The offset between the two
 * clocks is constant, so the shown time does not move while the screen ticks. On the web the core
 * clock is the clamped `Date.now()`.
 */
export function elapsedToWall(
  atElapsedMs: number,
  nowElapsedMs: number,
  nowWallMs: number,
): number {
  return nowWallMs - (nowElapsedMs - atElapsedMs);
}

/** The three results of the missed-sound report; an incomplete or absent report never counts. */
export type MissedResult = "none" | "some" | "unknown";

export function missedResultOf(m: MissedEventsUi | null): MissedResult {
  if (m === null || !m.complete || m.count === null) return "unknown";
  return m.count === 0 ? "none" : "some";
}

// ---- headline --------------------------------------------------------------------------------

export type Headline =
  | { kind: "all_quiet"; sinceWallMs: number }
  | { kind: "sound_since"; sinceWallMs: number }
  | { kind: "sound_at"; atWallMs: number; durationMs: number }
  | { kind: "sound_snoozed"; atWallMs: number }
  | { kind: "last_heard"; atWallMs: number }
  | { kind: "cant_hear" }
  | { kind: "no_contact" }
  | { kind: "monitoring" };

/**
 * Never "All quiet" while the link is not connected (alpha-ux-flows.md section 5, row 2), and never
 * while the baby device's microphone hears nothing: a silent room and a dead microphone look the
 * same on the strip, so the headline says which one it is.
 */
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
  if (p.peer.micSilent === true) return { kind: "cant_hear" };
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
 * stopped, then its battery (section 5). `dismissed` holds the kinds the user
 * closed for the current situation ("Stop sound" closes the noise alert, OK the others).
 */
export function fullScreenOf(
  p: ParentMonitorState,
  dismissed: ReadonlySet<FullScreenKind>,
): FullScreenKind | null {
  const link = p.link;
  const escalating = link.phase === "connecting" || link.phase === "reconnecting";
  // "I'm checking" does not close the screen: it stays red as "Alarm paused" until the link is back.
  if (escalating && link.level >= 2) return "link_lost";
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

// ---- silence ---------------------------------------------------------------------------------

const SILENCE_STATE_KEY: Readonly<Record<SilenceStatus, MessageKey>> = {
  silent: "native.ui.silence.silent",
  screen_may_light: "native.ui.silence.screenMayLight",
  may_buzz: "native.ui.silence.mayVibrate",
  may_ring: "native.ui.silence.mayRing",
  unknown: "native.ui.silence.unknown",
  no_access: "native.ui.silence.noAccess",
};

/**
 * The baby device's quiet state for the meta line under the strip. A browser baby device can
 * neither report nor change it, so it gets the plain "unknown, do it yourself" hint.
 */
export function silenceStateKey(peer: PeerStatusUi): MessageKey {
  return peer.isWebClient ? "webScreens.parent.quietUnknownWeb" : SILENCE_STATE_KEY[peer.silence];
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
