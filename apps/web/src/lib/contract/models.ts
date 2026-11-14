/*
 * The contract between the UI and the platform layer on the web. It mirrors `UiContract.kt`
 * (apps/android/ui-contract) model for model so both clients render the same state; the
 * `contract.test.ts` drift test reads the Kotlin file and fails when a field or enum member
 * appears on one side only. Enum members are lower_snake_case strings (`PEER_STOPPED` becomes
 * `peer_stopped`); fields keep their Kotlin names.
 *
 * Times: `*ElapsedMs` = `Date.now()` clamped never to go backwards (the core clock; durations,
 * countdowns); `*WallMs` = `Date.now()` (clock times shown to the user).
 */
import type { Locale } from "../i18n/bundles";

export type Role = "baby" | "parent";

/** Link state per ADR-017 section 7; `idle` = monitoring not started. */
export type LinkPhase =
  | "idle"
  | "connecting"
  | "connected"
  | "reconnecting"
  | "peer_stopped"
  | "failed_needs_user"
  | "stopped";

export type CauseHint =
  | "server_unreachable"
  | "peer_absent"
  | "media_blocked"
  /** Baby: no secret for the parent and the invite is over; show a new code. */
  | "pair_again"
  /** Parent: the baby device does not recognise this device; show a new code on it. */
  | "baby_unrecognised";

/**
 * A calm hint of the core (never an alarm): the clocks of the two devices differ by more than 10
 * minutes, or no mailbox was reachable at the last reachability check while the link is verified.
 */
export type LinkHint = "clock_skew" | "recovery_path_unavailable";

export type LinkQuality = "good" | "medium" | "poor" | "unknown";

export interface LinkUi {
  phase: LinkPhase;
  /** Escalation level 0..3 while not connected (ADR-017 section 7); 0 when connected. */
  level: number;
  /** Last contact with the peer; drives "not reachable for m:ss". */
  lastAliveElapsedMs: number | null;
  /** "I'm checking" mute end (parent, level 2 and above). */
  acknowledgedUntilElapsedMs: number | null;
  causeHint: CauseHint | null;
  /** Message key of a terminal failure, e.g. `pairing_key_mismatch`, `update_required`. */
  failureKey: string | null;
  /** Weak connection shown only when `poor`. */
  quality: LinkQuality;
  /** The baby device has been absent for over 10 minutes: a re-paired baby needs a new scan. */
  stalePairingHint: boolean;
  /** Calm hints currently raised by the core; the UI renders them later (tasks 5.3, 6.2). */
  hints: LinkHint[];
}

/** One value for the parent (ADR-018 section 3.3). */
export type SilenceStatus =
  "silent" | "screen_may_light" | "may_buzz" | "may_ring" | "unknown" | "no_access";

export interface PeerStatusUi {
  batteryPct: number | null;
  charging: boolean | null;
  silence: SilenceStatus;
  alarmsAllowed: boolean | null;
  nextAlarmMinutes: number | null;
  micSilent: boolean | null;
  /** The baby device is a browser: silence can never be read or set. */
  isWebClient: boolean;
  /** Wire names of the baby device's own warnings: "not_charging", "page_hidden", ... */
  unitWarnings: string[];
}

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

export interface MissedEventsUi {
  downMs: number;
  count: number | null;
  complete: boolean;
  /** End of the gap (wall clock); null = now. The start is `end - downMs`. */
  endWallMs: number | null;
  /** Loudest level 0..1 during the gap, when known. */
  loudest: number | null;
}

/** The last finished noise event, for "Sound at {time}, lasted {duration}". */
export interface SoundEventUi {
  atWallMs: number;
  durationMs: number;
}

export interface ParentMonitorState {
  link: LinkUi;
  peer: PeerStatusUi;
  /** Live level 0..1 of the received baby audio (meter). */
  level: number;
  /** Start of the current quiet period; null while disconnected or unknown (never "All quiet"). */
  quietSinceWallMs: number | null;
  noiseAlertActive: boolean;
  listen: ListenMode;
  snoozedUntilWallMs: number | null;
  readiness: ReadinessIssue[];
  silenceRequest: SilenceRequest;
  missed: MissedEventsUi | null;
  /** Set for a few seconds after recovery: "Connected again after m:ss". */
  restoredAfterMs: number | null;
  securityWarning: boolean;
  noiseSinceWallMs: number | null;
  noiseIsTest: boolean;
  /** The baby device's real noise threshold (from peer_status), so the meter can draw it. */
  babyThreshold: number | null;
  lastSound: SoundEventUi | null;
  /** The parent device's own battery percent, for "Your device is at {percent}". */
  ownBatteryPct: number | null;
  /** Android only: the core switched a discreet event to sound; always null in the browser. */
  noiseEscalatedAfterMin: number | null;
  /** Talk-back (`add-talk-back`). */
  talk: TalkUi;
  talkEnd: TalkEnd | null;
  talkPermission: TalkPermission;
  noisePause: NoisePause | null;
  talkUnconfirmed: boolean;
  talkAudibleUncertain: boolean;
  talkMicSilent: boolean;
  afterTalkListenUntilElapsedMs: number | null;
}

export interface BabyMonitorState {
  link: LinkUi;
  micLevel: number;
  threshold: number;
  holdSeconds: number;
  silenceApplied: boolean | null;
  dndAccess: boolean;
  kioskLocked: boolean;
  /** A previous DND rule was restored after a crash; show the notice once. */
  silenceRestoredNotice: boolean;
  micSilent: boolean;
  /** The parent is listening (shown on touch-wake only). */
  parentListening: boolean;
  /**
   * The name the pinned parent device sent over the verified DataChannel ("Connected to {name}");
   * null unless connected, empty when it sent none.
   */
  parentName: string | null;
  /** A device that is not the paired parent tried to connect (the core refused it). */
  securityWarning: boolean;
  /** A parent's voice plays now (static indicator under the edge). */
  parentTalking: boolean;
  /** The last finished talk of this run, local only. */
  lastTalk: LastTalkUi | null;
}

export type PairingPhase =
  "showing_code" | "expired" | "scanning" | "connecting" | "paired" | "failed";

export type PairingError =
  | "camera_denied"
  | "not_our_code"
  | "expired"
  | "key_mismatch"
  | "already_paired"
  | "unreachable"
  | "incomplete_paste";

export interface PairingState {
  phase: PairingPhase;
  /** Baby: QR / copyable payload. Never logged; see `makePairingState`. */
  codePayload: string | null;
  expiresAtElapsedMs: number | null;
  error: PairingError | null;
}

/**
 * Web only (no counterpart in UiContract.kt): the browser's own battery reading of this device,
 * from the Battery API where it exists. `pct` is 0..100; `charging` is null when unknown.
 */
export interface DeviceBattery {
  pct: number;
  charging: boolean | null;
}

/**
 * Web only: the state of the Screen Wake Lock. `off` = not monitoring (or not taken yet),
 * `lost` = the system or the browser released it or refused it, `unsupported` = no such API here.
 */
export type WakeLockState = "off" | "held" | "lost" | "unsupported";

/** Web only: this page was away while monitoring (hidden tab, or the device slept). */
export interface AwayReport {
  kind: "background" | "sleep";
  awayMs: number;
  /** When the page came back (wall clock). */
  endWallMs: number;
}

/**
 * Web only: what the browser did to this page while it monitors. Decisions about alarms stay in
 * the core; this only reports the page's own state so the screens can warn.
 */
export interface PageGuardState {
  wakeLock: WakeLockState;
  /** The page is not in front (another tab, a minimised window, a locked screen). */
  hidden: boolean;
  /** Set when the page returns from the background or from sleep; cleared by the user. */
  away: AwayReport | null;
}

/*
 * Talk-back (`add-talk-back`), mirroring the talk models of UiContract.kt: the same names and
 * shapes, lower-snake enum members and `kind`-tagged unions for the sealed `TalkUi`.
 */

/** Talk-back volume of the baby device (a baby setting). */
export type TalkVolume = "low" | "medium" | "high";

/** Why a talk ended on the parent device (core `TalkEndReason`). */
export type TalkEnd =
  | "released"
  | "limit"
  | "no_answer"
  | "link_lost"
  | "mic_failed"
  | "alert"
  | "switched_off"
  | "stopped";

/** Why the baby device refused a talk (core `TalkDenyReason`; `unknown` for a newer reason). */
export type TalkDeny =
  "off" | "unsupported" | "muted" | "busy" | "cooldown" | "budget" | "rate" | "unknown";

/** Why the talk button is disabled before a press. */
export type TalkUnavailable =
  "off" | "other_talking" | "unsupported" | "muted" | "not_connected" | "cooldown";

/** The talk button's state; times are on the elapsed clock. */
export type TalkUi =
  | { kind: "idle" }
  /** "One moment...": the request is on its way. */
  | { kind: "requesting" }
  /** "Speak now": the microphone is attached; the baby device ends the talk at `untilElapsedMs`. */
  | { kind: "talking"; untilElapsedMs: number }
  | { kind: "denied"; reason: TalkDeny; retryAtElapsedMs: number | null }
  | { kind: "unavailable"; reason: TalkUnavailable; retryAtElapsedMs: number | null };

/** Microphone permission step of the talk button (a press without permission sends nothing). */
export type TalkPermission = "none" | "needed" | "just_granted" | "refused";

/** Who pauses noise detection: this device's own talk, or the other parent device's. */
export type NoisePause = "you" | "other";

/**
 * Baby: the last talk, local only ("Last talk: {name}, {time}, {duration}"). `atWallMs` is the wall
 * time at which the talk started (its end minus `durationMs`), the same on every client.
 */
export interface LastTalkUi {
  /** The parent's name on the baby unit; empty when it sent none. */
  name: string;
  atWallMs: number;
  durationMs: number;
}

/** The talk fields of `ParentMonitorState`, kept apart so the talk model can own them. */
export interface ParentTalkFields {
  talk: TalkUi;
  /** How the last talk ended; null before the first and after the next press. */
  talkEnd: TalkEnd | null;
  talkPermission: TalkPermission;
  noisePause: NoisePause | null;
  /** The baby device's playback check could not hear the last voice ("may not be audible"). */
  talkUnconfirmed: boolean;
  /** Three unconfirmed talks in a row: shown before the next press, never a denial. */
  talkAudibleUncertain: boolean;
  /**
   * Your own microphone has picked up (almost) no sound for a while during the talk: shown
   * as a line, never spoken while the microphone is open.
   */
  talkMicSilent: boolean;
  /** The 10 s listen after a talk runs until then ("tap to keep listening"). */
  afterTalkListenUntilElapsedMs: number | null;
}

export function defaultParentTalkFields(): ParentTalkFields {
  return {
    talk: { kind: "idle" },
    talkEnd: null,
    talkPermission: "none",
    noisePause: null,
    talkUnconfirmed: false,
    talkAudibleUncertain: false,
    talkMicSilent: false,
    afterTalkListenUntilElapsedMs: null,
  };
}

/** The talk fields of `BabyMonitorState`. */
export interface BabyTalkFields {
  /** A parent's voice plays now (static indicator under the edge). */
  parentTalking: boolean;
  /** The last finished talk of this run, local only. */
  lastTalk: LastTalkUi | null;
}

/**
 * Web only (no counterpart in UiContract.kt): what the browser adds to the talk-back models.
 * `micUnsupported` = this browser cannot capture for a talk (parent); `soundSuspended` = the
 * browser suspended this device's audio, so talk-back is muted until a tap (baby). Android has
 * neither: its parent always has a microphone and its baby plays through the alarm stream.
 */
export interface WebTalkState {
  micUnsupported: boolean;
  soundSuspended: boolean;
}

export function defaultWebTalk(): WebTalkState {
  return { micUnsupported: false, soundSuspended: false };
}

export type Theme = "dark" | "light" | "system";

export type AlertOutput = "default" | "speaker_only";

/**
 * Mirrors `AlertStyle` and `DiscreetEscalation` of UiContract.kt. The browser does not offer the
 * discreet style (it cannot vibrate), so it always keeps "sound" and never reads these values.
 */
export type AlertStyle = "sound" | "discreet";
export type DiscreetEscalation = "one_minute" | "two_minutes" | "five_minutes" | "off";

export interface SettingsState {
  theme: Theme;
  /** null = follow the browser language. */
  language: Locale | null;
  threshold: number;
  holdSeconds: number;
  alertOutput: AlertOutput;
  alertStyle: AlertStyle;
  discreetEscalation: DiscreetEscalation;
  alertGainPct: number;
  silenceWhileMonitoring: boolean;
  advancedEnabled: boolean;
  signalingUrls: string[];
  /** "Only same Wi-Fi or my own server": no project server contacted from this device. */
  onlyLocal: boolean;
  turnUrl: string | null;
  turnUsername: string | null;
  /** Only the fact: the password itself is never part of any state. */
  turnPasswordSaved: boolean;
  heartbeatSeconds: number;
  /** Last successful test alert (settings line "Last test"). */
  lastTestWallMs: number | null;
  /** Baby: parents may talk through this device (switch of the single pairing; default on). */
  talkBackEnabled: boolean;
  talkVolume: TalkVolume;
  /** Baby: a soft chime announces a talk. */
  talkChime: boolean;
  /** Parent: the one-time "hold to talk" coach mark was shown. */
  talkCoachSeen: boolean;
}

/** Baby self-test result (ADR-010) shown on the setup checklist. */
export type DeviceCheck = "unknown" | "ok" | "limited" | "not_recommended";

/** Why a device check is not OK (one plain line each; worst first in `AppState`). */
export type DeviceCheckReason =
  | "mic_permission_missing"
  | "mic_silent"
  | "battery_unhealthy"
  | "battery_low_not_charging"
  | "no_mailbox_reachable"
  | "not_charging"
  | "battery_optimized"
  | "dnd_access_missing"
  | "some_mailboxes_unreachable";

/** Whether one mailbox answered the device check's connect probe (host only). */
export interface MailboxReach {
  host: string;
  reachable: boolean;
}

/**
 * One saved connection (pairing) of the session role (`multi-parent-pairings`). The web client
 * keeps one in-memory pairing and shows no list, so `AppState.connections` stays empty here.
 */
export interface ConnectionUi {
  /** The stable local id of the pairing. */
  id: string;
  /** The local connection name; null while there is none (a numbered default is shown). */
  name: string | null;
  isDefault: boolean;
  /** The connection monitoring uses (or will use) in this session. */
  selected: boolean;
}

export interface AppState {
  /** The session role; null shows the role screen. */
  role: Role | null;
  /** "Start mode": the role a start without monitoring opens in; null asks every time. */
  defaultRole: Role | null;
  onboardingDone: boolean;
  /** A saved pairing exists (home screen shows one-tap start). */
  paired: boolean;
  peerName: string | null;
  /** The saved connections of the session role; empty on the web (one in-memory pairing). */
  connections: ConnectionUi[];
  /** Fewer than the cap of pairings are saved: a further one can be added. */
  canAddPairing: boolean;
  monitoring: boolean;
  /** Parent: the running session was started by the automatic start (never on the web). */
  autoStarted: boolean;
  /** False while monitoring is on but nothing keeps the page alive: show red, never green. */
  backgroundProtection: boolean;
  lastUsedWallMs: number | null;
  /** Show the first-run coach marks on the parent monitoring screen once. */
  coachPending: boolean;
  deviceCheck: DeviceCheck;
  /** Why the device check is not OK, worst first; empty while unknown or OK. */
  deviceCheckReasons: DeviceCheckReason[];
  /** The device check's mailbox probes (host only). */
  deviceCheckMailboxes: MailboxReach[];
}

// Defaults equal the Kotlin constructor defaults.

export function defaultLinkUi(): LinkUi {
  return {
    phase: "idle",
    level: 0,
    lastAliveElapsedMs: null,
    acknowledgedUntilElapsedMs: null,
    causeHint: null,
    failureKey: null,
    quality: "unknown",
    stalePairingHint: false,
    hints: [],
  };
}

export function defaultPeerStatus(): PeerStatusUi {
  return {
    batteryPct: null,
    charging: null,
    silence: "unknown",
    alarmsAllowed: null,
    nextAlarmMinutes: null,
    micSilent: null,
    isWebClient: false,
    unitWarnings: [],
  };
}

export function defaultParentMonitor(): ParentMonitorState {
  return {
    link: defaultLinkUi(),
    peer: defaultPeerStatus(),
    level: 0,
    quietSinceWallMs: null,
    noiseAlertActive: false,
    listen: "off",
    snoozedUntilWallMs: null,
    readiness: [],
    silenceRequest: "idle",
    missed: null,
    restoredAfterMs: null,
    securityWarning: false,
    noiseSinceWallMs: null,
    noiseIsTest: false,
    babyThreshold: null,
    lastSound: null,
    ownBatteryPct: null,
    noiseEscalatedAfterMin: null,
    ...defaultParentTalkFields(),
  };
}

export function defaultBabyMonitor(): BabyMonitorState {
  return {
    link: defaultLinkUi(),
    micLevel: 0,
    threshold: 0.12,
    holdSeconds: 5,
    silenceApplied: null,
    dndAccess: false,
    kioskLocked: false,
    silenceRestoredNotice: false,
    micSilent: false,
    parentListening: false,
    parentName: null,
    securityWarning: false,
    parentTalking: false,
    lastTalk: null,
  };
}

export function defaultSettings(): SettingsState {
  return {
    theme: "dark",
    language: null,
    threshold: 0.12,
    holdSeconds: 5,
    alertOutput: "default",
    alertStyle: "sound",
    discreetEscalation: "two_minutes",
    alertGainPct: 100,
    silenceWhileMonitoring: true,
    advancedEnabled: false,
    signalingUrls: [],
    onlyLocal: false,
    turnUrl: null,
    turnUsername: null,
    turnPasswordSaved: false,
    heartbeatSeconds: 10,
    lastTestWallMs: null,
    talkBackEnabled: true,
    talkVolume: "low",
    talkChime: true,
    talkCoachSeen: false,
  };
}

export function defaultAppState(): AppState {
  return {
    role: null,
    defaultRole: null,
    onboardingDone: false,
    paired: false,
    peerName: null,
    connections: [],
    canAddPairing: true,
    monitoring: false,
    autoStarted: false,
    backgroundProtection: true,
    lastUsedWallMs: null,
    coachPending: false,
    deviceCheck: "unknown",
    deviceCheckReasons: [],
    deviceCheckMailboxes: [],
  };
}

/**
 * A pairing state whose string and JSON forms never contain the payload (it holds the PSK).
 * Spreading or copying the object drops the guard, so build every new state through this.
 */
export function makePairingState(
  phase: PairingPhase,
  fields: Partial<Omit<PairingState, "phase">> = {},
): PairingState {
  const state: PairingState = {
    phase,
    codePayload: fields.codePayload ?? null,
    expiresAtElapsedMs: fields.expiresAtElapsedMs ?? null,
    error: fields.error ?? null,
  };
  const redacted = (): Omit<PairingState, "codePayload"> & { codePayload: string | null } => ({
    phase: state.phase,
    codePayload: state.codePayload === null ? null : "<redacted>",
    expiresAtElapsedMs: state.expiresAtElapsedMs,
    error: state.error,
  });
  Object.defineProperties(state, {
    toJSON: { value: redacted, enumerable: false },
    toString: {
      value: () => `PairingState(${JSON.stringify(redacted())})`,
      enumerable: false,
    },
  });
  return state;
}
