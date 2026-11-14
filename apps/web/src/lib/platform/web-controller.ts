import type {
  CoreApi,
  DeviceStatus,
  IceServer,
  InviteMailbox,
  LinkConfig,
  LinkEvent,
  LinkState,
  LinkStatus,
} from "@guardedsleep/core-wasm";
import { readable, writable, type Readable } from "svelte/store";
import {
  defaultAppState,
  defaultBabyMonitor,
  defaultParentMonitor,
  defaultParentTalkFields,
  defaultWebTalk,
  defaultSettings,
  makePairingState,
  type AppController,
  type AppState,
  type BabyMonitorState,
  type BabyTalkFields,
  type DeviceBattery,
  type ListenMode,
  type PageGuardState,
  type PairingError,
  type PairingState,
  type ParentMonitorState,
  type ParentTalkFields,
  type Role,
  type SettingsState,
  type WebTalkState,
  type TurnSaveResult,
} from "../contract";
import { resolveTurnSave, turnSettings, type TurnEntry } from "../contract/turn";
import type { AlertSink } from "./alert-sink";
import type { SoundGate } from "./audio";
import { BabyModel } from "./baby-model";
import type { Clock, Timers } from "./clock";
import { encodeBase64Url } from "./encoding";
import { toRtcIceServer } from "./ice-servers";
import { mailboxesForUrls, signalingUrlsOf } from "./invite-mailboxes";
import { LinkDriver, type LinkListener } from "./link-driver";
import type { InboundAudioStats, MediaEngine } from "./media-engine";
import { idleWatch, type PageWatch } from "./page-watch";
import { BabyTalkModel, ParentTalkModel, preemptingAlerts } from "./talk-model";
import type { MicrophonePermission } from "./talk-audio";
import { ParentModel } from "./parent-model";
import {
  ALARM_AFTER_MS,
  INVITE_VALIDITY_MS,
  MAX_HEARTBEAT_MS,
  MIN_HEARTBEAT_MS,
  PAIRING_UNREACHABLE_AFTER_MS,
  PSK_BYTES,
} from "./policy";
import type { SignalingFactory } from "./signaling";
import { phaseOf } from "./status-mapping";

/** Tells the controller when the network came back (the core then retries at once). */
export interface NetworkWatcher {
  start(onAvailable: () => void): void;
  stop(): void;
}

/** The browser's microphone for talk-back (parent): the support check, the permission, the prompt. */
export interface TalkMicPort {
  /** False in iOS Safari and on a page without media devices: the button says "not supported". */
  supported: boolean;
  query(): Promise<MicrophonePermission>;
  /** The permission step: asks once and closes the microphone again at once. */
  request(): Promise<"granted" | "refused">;
}

export interface ControllerDeps {
  core: Pick<CoreApi, "createLink" | "buildInvite" | "parseInvite" | "filterOnlyLocal">;
  clock: Clock;
  timers: Timers;
  signaling: SignalingFactory;
  /**
   * Opens the microphone (baby) or builds the playout chain (parent); rejects on a refusal.
   * `userIceServers` is the user-entered TURN (credentials included, memory only); the engine
   * appends it to the core's ICE list of every attempt.
   */
  createMedia(role: Role, userIceServers: RTCIceServer[]): Promise<MediaEngine>;
  gate: SoundGate;
  alerts: AlertSink;
  network: NetworkWatcher;
  randomBytes(length: number): Uint8Array;
  /** 53 random bits for the core's jitter. */
  entropy(): number;
  /** Where the invite of a NEW pairing points when this build does not offer the project server. */
  defaultMailboxUrls(): string[];
  /**
   * The project server (generated `PROJECT_SERVER`): `mailboxUrl` is its relay address, `host` its
   * lower-case DNS name (what the core matches for `only_local`), `enabled` the build flag
   * (`projectServerEnabled`, design section 8). Absent in tests without one.
   */
  projectServer?: { mailboxUrl: string; host: string; enabled: boolean };
  /**
   * Whether the deployment CSP `connect-src` lets the page open this address; an address it does
   * not list is ignored in invites and updates (design section 5, version skew). Absent: all pass.
   */
  connectAllowed?: (url: string) => boolean;
  /** Baby: the browser's own state (battery, page visibility, ...). A browser reports no ringer. */
  deviceStatus?: () => DeviceStatus;
  /** The device's battery for the baby screen; absent where the browser has no Battery API. */
  battery?: Readable<DeviceBattery | null>;
  /**
   * Parent: the name shown on the baby device ("Chrome on Android"), sent over the verified
   * DataChannel only. The browser family and system, never an identifier or a fingerprint.
   */
  deviceName?: () => string;
  /** Development builds only: the core also accepts loopback mailboxes in a received update. */
  allowDevLoopback?: boolean;
  /** Parent: talk-back's microphone permission; absent = talk-back is "not supported" here. */
  talkMic?: TalkMicPort;
  /** Wake lock, page visibility and sleep detection; absent in tests that do not need them. */
  page?: PageWatch;
}

/** Pairing material of this page. In memory only: never storage, URLs, logs or the DOM. */
interface StoredPairing {
  pskB64Url: string;
  /** The pairing's mailbox set (invite, then mailbox updates): the only source for the core. */
  urls: string[];
  pairedBefore: boolean;
  /** The pinned parent secret (64 hex characters): memory only, like the PSK. */
  parentSecret: string | null;
  /** Parent: the end of the first-pairing window (Unix seconds) while the baby has not acknowledged. */
  pinUntilS: number | null;
}

interface PendingPairing {
  pskB64Url: string;
  urls: string[];
  startedAt: number;
  expiresAtElapsedMs: number | null;
  unreachableShown: boolean;
  /** The parent secret of a stored pairing (a pending first pairing has none yet). */
  parentSecret: string | null;
  /**
   * What the core reported before the first verification saved the pairing (the events can come
   * before or after the `connected` status).
   */
  pinnedSecret: string | null;
  receivedUrls: string[] | null;
  /**
   * The first-pairing window (Unix seconds): the expiry of the invite, then the last
   * `window_until_s` of `parent_pinned`. Null once the baby acknowledged the secret.
   */
  pinUntilS: number | null;
}

interface Run {
  id: number;
  role: Role;
  pending: PendingPairing | null;
  monitoring: boolean;
  driver: LinkDriver | null;
}

/** Web-only failure keys of a run that could not start (no core key exists for them). */
/** Length of the core's CSPRNG seed (the seed field of the link config, sent as hex). */
const SEED_BYTES = 32;

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

const MIC_DENIED = "mic_denied";
const MIC_UNAVAILABLE = "mic_unavailable";

/**
 * The platform layer's single entry point for the UI (the web `LinkController`). Owns at most one
 * `LinkDriver` (one core `Link`) per monitoring or pairing run, keeps the page's choices in
 * memory, and maps the core's status and events onto the contract stores. Decisions about the
 * link itself stay in the core.
 */
export class WebController implements AppController {
  private readonly appStore = writable<AppState>(defaultAppState());
  private readonly settingsStore = writable<SettingsState>(defaultSettings());
  private readonly pairingStore = writable<PairingState | null>(null);
  private readonly soundStore = writable(false);
  private readonly parentModel: ParentModel;
  private readonly babyModel: BabyModel;
  private readonly parentStore = writable<ParentMonitorState>(defaultParentMonitor());
  private readonly babyStore = writable<BabyMonitorState>(defaultBabyMonitor());
  private readonly webTalkStore = writable<WebTalkState>(defaultWebTalk());
  // The models publish their own part; the stores always carry the merged state.
  private parentBase: ParentMonitorState = defaultParentMonitor();
  private parentTalk: ParentTalkFields = defaultParentTalkFields();
  private babyBase: BabyMonitorState = defaultBabyMonitor();
  private babyTalk: BabyTalkFields = { parentTalking: false, lastTalk: null };
  private readonly talkModel: ParentTalkModel;
  private readonly babyTalkModel: BabyTalkModel;
  private readonly alerts: AlertSink;

  readonly app: Readable<AppState> = this.appStore;
  readonly settings: Readable<SettingsState> = this.settingsStore;
  readonly pairing: Readable<PairingState | null> = this.pairingStore;
  readonly soundUnlocked: Readable<boolean> = this.soundStore;
  readonly parent: Readable<ParentMonitorState> = this.parentStore;
  readonly baby: Readable<BabyMonitorState> = this.babyStore;
  readonly webTalk: Readable<WebTalkState> = this.webTalkStore;
  readonly deviceBattery: Readable<DeviceBattery | null>;
  readonly pageGuard: Readable<PageGuardState>;

  private readonly page: PageWatch;
  private appState: AppState = defaultAppState();
  private settingsState: SettingsState = defaultSettings();
  private stored: StoredPairing | null = null;
  /** The TURN entry with its password: here only, never in a store, a log or browser storage. */
  private turn: TurnEntry | null = null;
  private run: Run | null = null;
  private runCounter = 0;

  constructor(private readonly deps: ControllerDeps) {
    this.deviceBattery = deps.battery ?? readable<DeviceBattery | null>(null);
    this.page = deps.page ?? idleWatch();
    this.pageGuard = this.page.state;
    // After a sleep or a return from the background the sockets are likely dead: the core retries
    // at once on this existing input and decides everything else itself.
    this.page.onResumed(() => this.run?.driver?.input({ type: "network_available" }));
    this.alerts = preemptingAlerts(deps.alerts, () => this.preemptTalk());
    this.parentModel = new ParentModel(
      deps.clock,
      this.alerts,
      {
        muteRemote: (muted) => this.run?.driver?.media((m) => m.setPlaybackMuted(muted)),
        listenStopped: () => this.run?.driver?.input({ type: "set_listen", enabled: false }),
        testAlertArrived: (wallMs) => this.updateSettingsState({ lastTestWallMs: wallMs }),
        snoozeEnded: () => undefined,
      },
      () => this.settingsState.threshold,
      (state) => {
        this.parentBase = state;
        this.publishParent();
      },
    );
    this.babyModel = new BabyModel(deps.clock, (state) => {
      this.babyBase = state;
      this.publishBaby();
    });
    this.talkModel = new ParentTalkModel(deps.clock, (fields) => {
      this.parentTalk = fields;
      this.publishParent();
    });
    this.talkModel.setBrowserSupported(deps.talkMic?.supported ?? false);
    this.webTalkStore.set({
      ...defaultWebTalk(),
      micUnsupported: !(deps.talkMic?.supported ?? false),
    });
    this.babyTalkModel = new BabyTalkModel(
      deps.clock,
      () => this.babyModel.parentDisplayName,
      (fields) => {
        this.babyTalk = fields;
        this.publishBaby();
      },
    );
    // A page that leaves the front must not keep the microphone: the talk ends at once.
    this.page.state.subscribe((guard) => {
      if (guard.hidden) this.interruptTalk();
    });
    this.babyModel.setSettings(this.settingsState.threshold, this.settingsState.holdSeconds);
    deps.gate.onChange((unlocked) => {
      this.soundStore.set(unlocked);
      this.onSoundState(unlocked);
    });
  }

  // ---- app ---------------------------------------------------------------------------------

  // The web client keeps one in-memory pairing, so a role change still forgets it; per-role
  // pairings and a stored start mode are the web-parity IDEA in docs/backlog.md.
  chooseRole(role: Role, makeDefault = false): void {
    if (this.appState.role !== null && this.appState.role !== role) this.forgetPairing();
    this.setApp({ role, defaultRole: makeDefault ? role : this.appState.defaultRole });
  }

  switchMode(): void {
    if (this.appState.monitoring) return;
    this.cancelPairing();
    this.setApp({ role: null });
  }

  setDefaultRole(role: Role | null): void {
    this.setApp({ defaultRole: role });
  }

  // The web client has no baby device check yet (browser facts differ); `deviceCheck` stays
  // "unknown" and no screen shows it.
  runDeviceCheck(): void {}

  // Named connections need saved pairings; the web keeps one in-memory pairing and an empty
  // connection list, so these are no-ops (web parity is the IDEA in docs/backlog.md).
  selectConnection(_id: string): void {}

  renameConnection(_id: string, _name: string): boolean {
    return false;
  }

  setDefaultConnection(_id: string): void {}

  forgetConnection(_id: string): void {}

  completeOnboarding(): void {
    this.setApp({ onboardingDone: true });
  }

  updateSettings(transform: (settings: SettingsState) => SettingsState): void {
    const before = this.settingsState;
    const after = { ...transform(before), ...turnSettings(this.turn) };
    if (JSON.stringify(after) === JSON.stringify(before)) return;
    this.settingsState = after;
    this.settingsStore.set(after);
    this.babyModel.setSettings(after.threshold, after.holdSeconds);
    const r = this.run;
    if (!r?.driver) return;
    if (
      r.role === "baby" &&
      (after.threshold !== before.threshold || after.holdSeconds !== before.holdSeconds)
    ) {
      r.driver.input({
        type: "set_noise_gate",
        threshold: after.threshold,
        hold_ms: after.holdSeconds * 1000,
      });
    }
    if (after.alertGainPct !== before.alertGainPct) {
      r.driver.media((m) => m.setPlaybackGainPct(after.alertGainPct));
    }
    if (
      after.talkBackEnabled !== before.talkBackEnabled ||
      after.talkVolume !== before.talkVolume ||
      after.talkChime !== before.talkChime
    ) {
      this.sendTalkSettings();
    }
  }

  setTurnServer(url: string, username: string, password: string): TurnSaveResult {
    const { result, entry } = resolveTurnSave(this.turn, url, username, password);
    this.turn = entry;
    this.updateSettingsState(turnSettings(entry));
    return result;
  }

  clearTurnServer(): void {
    this.turn = null;
    this.updateSettingsState(turnSettings(null));
  }

  forgetPairing(): void {
    this.stopMonitoring();
    this.cancelPairing();
    this.stored = null;
    this.parentModel.cancelSnooze();
    this.setApp({ paired: false, peerName: null });
  }

  unlockSound(): void {
    void this.deps.gate.unlock();
  }

  dismissCoach(): void {
    this.setApp({ coachPending: false });
  }

  // ---- pairing -----------------------------------------------------------------------------

  startPairing(): void {
    if (this.appState.role !== "baby") return;
    this.deps.gate.unlock().catch(() => undefined);
    this.cancelPairing();
    this.stopRun(true);
    const urls = this.newPairingUrls();
    const now = this.deps.clock.elapsedMs();
    const psk = this.deps.randomBytes(PSK_BYTES);
    const expiresAtS = Math.floor(this.deps.clock.wallMs() / 1000) + INVITE_VALIDITY_MS / 1000;
    const invite = urls.length
      ? this.deps.core.buildInvite(psk, this.withOnlyLocal(mailboxesForUrls(urls)), expiresAtS)
      : null;
    const pskB64Url = encodeBase64Url(psk);
    psk.fill(0);
    if (!invite?.ok) {
      this.pairingStore.set(makePairingState("failed", { error: "unreachable" }));
      return;
    }
    const pending: PendingPairing = {
      pskB64Url,
      urls,
      startedAt: now,
      expiresAtElapsedMs: now + INVITE_VALIDITY_MS,
      unreachableShown: false,
      parentSecret: null,
      pinnedSecret: null,
      receivedUrls: null,
      pinUntilS: expiresAtS,
    };
    this.pairingStore.set(
      makePairingState("showing_code", {
        codePayload: invite.text,
        expiresAtElapsedMs: pending.expiresAtElapsedMs,
      }),
    );
    void this.startRun("baby", pending, false, false);
  }

  refreshPairingCode(): void {
    this.startPairing();
  }

  cancelPairing(): void {
    if (this.run?.pending) this.stopRun(true);
    this.pairingStore.set(null);
  }

  submitPairingCode(payload: string): void {
    if (this.appState.role !== "parent") return;
    this.deps.gate.unlock().catch(() => undefined);
    const parsed = this.deps.core.parseInvite(payload, Math.floor(this.deps.clock.wallMs() / 1000));
    if (!parsed.ok) {
      this.pairingStore.set(makePairingState("failed", { error: parsed.error }));
      return;
    }
    this.stopRun(true);
    const pskB64Url = encodeBase64Url(parsed.invite.psk);
    parsed.invite.psk.fill(0);
    this.pairingStore.set(makePairingState("connecting"));
    // Kept only after the first verification, so a wrong code never leaves a pairing behind.
    void this.startRun(
      "parent",
      {
        pskB64Url,
        urls: signalingUrlsOf(parsed.invite.mailboxes),
        startedAt: this.deps.clock.elapsedMs(),
        expiresAtElapsedMs: null,
        unreachableShown: false,
        parentSecret: null,
        pinnedSecret: null,
        receivedUrls: null,
        pinUntilS: parsed.invite.expiresAtEpochS,
      },
      false,
      false,
    );
  }

  // ---- monitoring --------------------------------------------------------------------------

  startMonitoring(): void {
    this.deps.gate.unlock().catch(() => undefined);
    const role = this.appState.role;
    if (role === null) return;
    const existing = this.run;
    if (existing) {
      // The pairing link becomes the monitoring run; the other device never sees a gap.
      existing.monitoring = true;
      this.setApp({ monitoring: true });
      return;
    }
    const stored = this.stored;
    if (!stored) return;
    this.setApp({ monitoring: true });
    void this.startRun(
      role,
      {
        pskB64Url: stored.pskB64Url,
        urls: stored.urls,
        startedAt: this.deps.clock.elapsedMs(),
        expiresAtElapsedMs: null,
        unreachableShown: false,
        parentSecret: stored.parentSecret,
        pinnedSecret: null,
        receivedUrls: null,
        pinUntilS: stored.pinUntilS,
      },
      stored.pairedBefore,
      true,
    );
  }

  stopMonitoring(): void {
    this.setApp({ monitoring: false });
    this.stopRun(true);
  }

  // ---- parent intents ----------------------------------------------------------------------

  setListen(mode: ListenMode): void {
    const r = this.run;
    if (!r?.driver) return;
    if (this.parentModel.setListen(mode, this.deps.clock.elapsedMs())) {
      if (mode !== "off") this.talkModel.clearAfterListen();
      r.driver.input({ type: "set_listen", enabled: mode !== "off" });
    }
  }

  requestTestAlert(): void {
    const driver = this.run?.driver;
    if (driver && this.parentModel.state.link.phase === "connected") {
      driver.input({ type: "request_probe" });
    } else {
      // Tested this device only: the chain runs locally with the baby device not involved.
      this.alerts.noiseAlert("new", true, false);
      this.updateSettingsState({ lastTestWallMs: this.deps.clock.wallMs() });
    }
  }

  snooze(minutes: number): void {
    this.parentModel.snooze(minutes);
    const until = this.parentModel.snoozeUntilElapsedMs();
    if (until !== null) this.run?.driver?.input({ type: "set_snooze", until_ms: until });
  }

  cancelSnooze(): void {
    this.parentModel.cancelSnooze();
    this.run?.driver?.input({ type: "set_snooze", until_ms: null });
  }

  acknowledgeAlarm(): void {
    this.run?.driver?.input({ type: "acknowledge_alarm" });
  }

  stopAlertSound(): void {
    this.parentModel.stopAlertSound();
  }

  requestSilenceBaby(): void {
    const driver = this.run?.driver;
    if (!driver) return;
    this.parentModel.requestSilence(this.deps.clock.elapsedMs());
    driver.input({ type: "request_quiet" });
  }

  tryReconnectNow(): void {
    this.run?.driver?.input({ type: "network_available" });
  }

  dismissMissedEvents(): void {
    this.parentModel.dismissMissed();
  }

  retryKeepAwake(): void {
    this.page.retryKeepAwake();
  }

  dismissAwayReport(): void {
    this.page.dismissAway();
  }

  reportTestResult(heardAndFelt: boolean): void {
    if (heardAndFelt) this.updateSettingsState({ lastTestWallMs: this.deps.clock.wallMs() });
  }

  // ---- talk-back (parent) ------------------------------------------------------------------

  setTalk(pressed: boolean): void {
    if (pressed) this.talkPressed();
    else this.talkReleased();
  }

  private talkPressed(): void {
    const driver = this.run?.driver;
    if (!driver || this.run?.role !== "parent") return;
    if (this.talkModel.busy || !this.talkModel.available) return;
    const permission = this.talkModel.permission;
    if (permission !== "granted" && permission !== "done") {
      // The permission step comes before the first talk, never during a hold: nothing is sent.
      this.talkModel.setPermission("sheet");
      return;
    }
    // Warm start: the browser opens the microphone now; it is attached only at the grant.
    driver.media((m) => void m.warmTalkMic());
    driver.input({ type: "set_talk", pressed: true });
  }

  private talkReleased(): void {
    const driver = this.run?.driver;
    if (!driver || this.run?.role !== "parent" || !this.talkModel.busy) return;
    driver.input({ type: "set_talk", pressed: false });
    // Released before the grant: the warm microphone was never attached; close it now.
    driver.media((m) => m.discardTalkMic());
  }

  interruptTalk(): void {
    this.talkReleased();
  }

  requestTalkPermission(): void {
    const port = this.deps.talkMic;
    if (!port) return;
    void port.request().then((result) => this.talkPermissionResult(result === "granted"));
  }

  talkPermissionResult(granted: boolean): void {
    this.talkModel.setPermission(granted ? "done" : "refused");
  }

  dismissTalkPermission(): void {
    const permission = this.talkModel.permission;
    this.talkModel.setPermission(permission === "done" ? "granted" : "unknown");
  }

  dismissTalkCoach(): void {
    this.updateSettingsState({ talkCoachSeen: true });
  }

  /** An alert is about to sound on this device: a talk in progress ends first. */
  private preemptTalk(): void {
    if (this.talkModel.busy) this.run?.driver?.input({ type: "alert_preempts_talk" });
  }

  private refreshTalkPermission(run: Run): void {
    const port = this.deps.talkMic;
    if (!port?.supported) return;
    void port.query().then((state) => {
      if (this.run === run && state === "granted") this.talkModel.setPermission("granted");
    });
  }

  // ---- talk-back (baby) --------------------------------------------------------------------

  playTalkChimeSample(): void {
    this.deps.gate.unlock().catch(() => undefined);
    const level = this.settingsState.talkVolume;
    this.run?.driver?.media((m) => m.playTalkChime(level, () => undefined));
  }

  talkSettingsOpened(): void {
    this.run?.driver?.input({ type: "talk_settings_opened" });
  }

  private sendTalkSettings(): void {
    const driver = this.run?.driver;
    if (!driver || this.run?.role !== "baby") return;
    const { talkBackEnabled: enabled, talkVolume: level, talkChime: chime } = this.settingsState;
    driver.input({ type: "set_talk_back", enabled, level, chime });
  }

  /**
   * The browser suspended or resumed this device's audio. A suspended baby cannot play a talk, so
   * the core reports `muted` and the night screen shows how to turn sound back on.
   */
  private onSoundState(unlocked: boolean): void {
    const r = this.run;
    if (r?.role !== "baby") return;
    this.webTalkStore.update((w) => ({ ...w, soundSuspended: !unlocked }));
    r.driver?.input({ type: "set_talk_muted", muted: !unlocked });
  }

  // ---- baby intents ------------------------------------------------------------------------

  setKioskLocked(locked: boolean): void {
    this.babyModel.setKioskLocked(locked);
  }

  dismissSilenceRestoredNotice(): void {
    this.babyModel.setSilenceRestoredNotice(false);
  }

  // ---- runs --------------------------------------------------------------------------------

  private async startRun(
    role: Role,
    pending: PendingPairing,
    pairedBefore: boolean,
    monitoring: boolean,
  ): Promise<void> {
    this.stopRun(false);
    const run: Run = {
      id: ++this.runCounter,
      role,
      pending: monitoring ? null : pending,
      monitoring,
      driver: null,
    };
    this.run = run;
    this.parentModel.reset();
    this.babyModel.reset();
    this.talkModel.reset();
    this.babyTalkModel.reset();
    this.webTalkStore.update((w) => ({ ...w, soundSuspended: false }));

    let media: MediaEngine;
    try {
      media = await this.deps.createMedia(role, userTurnServers(this.turn));
    } catch (e) {
      if (this.run === run) this.onMediaFailed(run, e);
      return;
    }
    if (this.run !== run) {
      // Cancelled while the browser asked for the microphone.
      media.release();
      return;
    }

    const settings = this.settingsState;
    const driver = new LinkDriver({
      config: this.buildConfig(role, pending, pairedBefore, settings),
      core: this.deps.core,
      signaling: this.deps.signaling,
      media,
      clock: this.deps.clock,
      timers: this.deps.timers,
      listener: this.listenerFor(run),
      entropy: this.deps.entropy(),
      mailboxAllowed: this.mailboxAllowed,
      deviceStatus: role === "baby" ? (this.deps.deviceStatus ?? (() => ({}))) : undefined,
      // A browser cannot silence its device; the parent then sees "unknown", never a promise.
      applyQuiet: role === "baby" ? (this.deps.deviceStatus ?? (() => ({}))) : undefined,
    });
    run.driver = driver;
    this.deps.network.start(() => driver.input({ type: "network_available" }));
    driver.start();
    if (role === "baby") {
      // A browser captures raw (no echo cancellation), which the playback check needs; the
      // settings are this page's; a suspended audio context cannot play a talk.
      driver.input({ type: "set_raw_capture", raw: true });
      this.sendTalkSettings();
      this.webTalkStore.update((w) => ({ ...w, soundSuspended: !this.deps.gate.unlocked }));
      driver.input({ type: "set_talk_muted", muted: !this.deps.gate.unlocked });
    } else {
      this.refreshTalkPermission(run);
    }
    if (role === "parent") {
      driver.media((m) => m.setPlaybackGainPct(settings.alertGainPct));
      const snoozeUntil = this.parentModel.snoozeUntilElapsedMs();
      if (snoozeUntil !== null) driver.input({ type: "set_snooze", until_ms: snoozeUntil });
    }
  }

  private onMediaFailed(run: Run, error: unknown): void {
    this.run = null;
    this.alerts.clear();
    if (run.role === "baby") {
      const name = error instanceof DOMException ? error.name : "";
      this.babyModel.fail(
        name === "NotAllowedError" || name === "SecurityError" ? MIC_DENIED : MIC_UNAVAILABLE,
      );
    }
    if (run.pending) {
      // The detail (microphone) is on the baby state; pairing only learns that it cannot go on.
      this.pairingStore.set(makePairingState("failed", { error: "unreachable" }));
    }
  }

  private stopRun(userStop: boolean): void {
    const r = this.run;
    if (!r) return;
    this.run = null;
    this.deps.network.stop();
    if (userStop) r.driver?.stop();
    else r.driver?.abort();
    this.parentModel.reset();
    this.babyModel.reset();
    this.talkModel.reset();
    this.babyTalkModel.reset();
    this.alerts.clear();
  }

  private endPendingRun(r: Run): void {
    if (this.run !== r) return;
    this.run = null;
    this.deps.network.stop();
    r.driver?.stop();
    this.parentModel.reset();
    this.babyModel.reset();
    this.talkModel.reset();
    this.babyTalkModel.reset();
  }

  private buildConfig(
    role: Role,
    pending: PendingPairing,
    pairedBefore: boolean,
    s: SettingsState,
  ): LinkConfig {
    const heartbeat = Math.min(
      MAX_HEARTBEAT_MS,
      Math.max(MIN_HEARTBEAT_MS, s.heartbeatSeconds * 1000),
    );
    const liveness = 2 * heartbeat + 5_000;
    // The core rejects a link whose loss detection is not at least 20 s ahead of the alarm.
    const alarmAfter = Math.max(ALARM_AFTER_MS, liveness + 20_000);
    return {
      role,
      psk_b64url: pending.pskB64Url,
      // Fresh for every run: the core derives every nonce from it, so a reused seed would repeat them.
      seed: toHex(this.deps.randomBytes(SEED_BYTES)),
      paired_before: pairedBefore,
      heartbeat_interval_ms: heartbeat,
      alarm_after_ms: alarmAfter,
      noise_threshold: s.threshold,
      noise_hold_ms: s.holdSeconds * 1000,
      ...(pending.parentSecret === null ? {} : { pinned_parent: pending.parentSecret }),
      ...(pending.pinUntilS === null ? {} : { pin_window_until_s: pending.pinUntilS }),
      ...(role === "parent" ? { device_name: this.deps.deviceName?.() ?? "" } : {}),
      // The relay mailboxes of the pairing record: the core opens each at its own address, and a
      // record without any works LAN-only (no fallback to a default or a setting; the owner
      // decision of 2026-11-07). The baby also sends the set to the parent after each verification.
      mailboxes: this.withOnlyLocal(mailboxesForUrls(pending.urls)).filter((m) =>
        this.mailboxAllowed(m.url),
      ),
      allow_dev_loopback: this.deps.allowDevLoopback ?? false,
      // The core drops the relay entries of `project_host` only when it gets the host: `only_local`
      // alone filters nothing. Takes effect from the next run.
      only_local: s.onlyLocal,
      ...(s.onlyLocal && this.deps.projectServer
        ? { project_host: this.deps.projectServer.host }
        : {}),
    };
  }

  /**
   * The mailboxes of a NEW pairing (baby only): the advanced list when set, else the project server
   * when the build offers it and "Only same Wi-Fi or my own server" is off, else the deployment's
   * own `/signaling`. Existing records never consult this.
   */
  private newPairingUrls(): string[] {
    const s = this.settingsState;
    if (s.advancedEnabled && s.signalingUrls.length > 0) return s.signalingUrls;
    const project = this.deps.projectServer;
    if (project?.enabled && !s.onlyLocal) return [project.mailboxUrl];
    return this.deps.defaultMailboxUrls();
  }

  /** The deployment CSP as the page's own check (version skew); used for the config and the driver. */
  private readonly mailboxAllowed = (url: string): boolean =>
    this.deps.connectAllowed?.(url) ?? true;

  /**
   * The ONE place where "Only same Wi-Fi or my own server" (`SettingsState.onlyLocal`) filters the
   * project server out of a mailbox set (a new invite, an invite received here); the core owns the
   * rule. Without a project server (a build or test that has none) there is nothing to drop.
   */
  private withOnlyLocal(mailboxes: InviteMailbox[]): InviteMailbox[] {
    const project = this.deps.projectServer;
    if (!this.settingsState.onlyLocal || !project) return mailboxes;
    return this.deps.core.filterOnlyLocal(mailboxes, project.host);
  }

  // ---- core callbacks ----------------------------------------------------------------------

  private listenerFor(r: Run): LinkListener {
    const current = (): boolean => this.run === r;
    return {
      onStatus: (status: LinkStatus, now: number) => {
        if (!current()) return;
        if (r.role === "parent") {
          this.parentModel.onStatus(status, now);
          this.talkModel.setConnected(phaseOf(status.state) === "connected");
        } else this.babyModel.onStatus(status);
        this.onPairingStatus(r, status);
      },
      onEvent: (event: LinkEvent, now: number) => {
        if (!current()) return;
        // The one event that carries the parent secret: kept in memory next to the PSK, never logged.
        if (event.type === "parent_pinned")
          this.keepParentSecret(r, event.secret, event.window_until_s ?? null);
        else if (event.type === "mailboxes_updated") this.keepMailboxes(r, event.mailboxes);
        if (r.role === "parent") {
          this.parentModel.onEvent(event, now);
          this.talkModel.onEvent(event, now);
          // A microphone that failed may have lost its permission: ask again before the next talk.
          if (
            event.type === "talk_changed" &&
            event.state.state === "ended" &&
            event.state.reason === "mic_failed"
          ) {
            this.talkModel.setPermission("unknown");
            this.refreshTalkPermission(r);
          }
        } else {
          this.babyModel.onEvent(event);
          this.babyTalkModel.onEvent(event);
        }
      },
      onTick: (now: number) => {
        if (!current()) return;
        if (r.role === "parent") {
          this.parentModel.onTick(now);
          this.talkModel.onTick(now);
        }
        this.checkPairingTimers(r, now);
      },
      onMicLevel: (rms: number, now: number) => {
        if (current()) this.babyModel.onMicLevel(rms, now);
      },
      onRemoteLevel: (rms: number, now: number) => {
        if (current()) this.parentModel.onRemoteLevel(rms, now);
      },
      onTalkMicLevel: (rms: number, now: number) => {
        if (current()) this.talkModel.onMicLevel(rms, now);
      },
      onInboundStats: (stats: InboundAudioStats, now: number) => {
        if (current()) this.parentModel.onInboundStats(stats, now);
      },
      onTornDown: () => undefined,
    };
  }

  private onPairingStatus(r: Run, status: LinkStatus): void {
    const pending = r.pending;
    if (!pending) return;
    const state: LinkState = status.state;
    // The baby counts the pairing as complete only when it pinned the parent (`parent_pinned`),
    // not when the link connects: an unpinned baby has no parent.
    if (state === "connected") {
      if (r.role === "parent") this.completePairing(r, pending);
    } else if (state === "failed_needs_user") {
      const error: PairingError =
        status.key === "pairing_key_mismatch" ? "key_mismatch" : "unreachable";
      this.pairingStore.set(makePairingState("failed", { error }));
      this.endPendingRun(r);
    }
  }

  /** The pairing is kept (in memory) and the pairing link becomes the monitoring run. */
  private completePairing(r: Run, pending: PendingPairing): void {
    this.stored = {
      pskB64Url: pending.pskB64Url,
      urls: pending.receivedUrls ?? pending.urls,
      pairedBefore: true,
      parentSecret: pending.pinnedSecret,
      pinUntilS: pending.pinUntilS,
    };
    r.pending = null;
    r.monitoring = true;
    // The pairing link becomes the monitoring run; the other device never sees a gap.
    this.setApp({ paired: true, monitoring: true });
    // Last, so a UI that reacts to "paired" already sees a consistent monitoring state.
    this.pairingStore.set(makePairingState("paired"));
  }

  /**
   * `parent_pinned`: the secret (and the first-pairing window) next to the PSK, in memory. Before
   * the first verification saved the pairing they wait in the run; the baby completes its pairing
   * here, at the pin.
   */
  private keepParentSecret(r: Run, secret: string, untilS: number | null): void {
    if (r.pending) {
      r.pending.pinnedSecret = secret;
      r.pending.pinUntilS = untilS;
      if (r.role === "baby") this.completePairing(r, r.pending);
    } else if (this.stored) {
      this.stored = { ...this.stored, parentSecret: secret, pinUntilS: untilS };
    }
  }

  /** The baby's current mailbox set replaces the stored one. */
  private keepMailboxes(r: Run, mailboxes: InviteMailbox[]): void {
    const urls = signalingUrlsOf(mailboxes);
    if (urls.length === 0) return;
    if (r.pending) {
      r.pending.receivedUrls = urls;
    } else if (this.stored) {
      this.stored = { ...this.stored, urls };
    }
  }

  private checkPairingTimers(r: Run, now: number): void {
    const pending = r.pending;
    if (!pending) return;
    const expiry = pending.expiresAtElapsedMs;
    if (r.role === "baby" && expiry !== null && now >= expiry) {
      // The code is dead: close the room so nobody can still use it.
      this.pairingStore.set(makePairingState("expired"));
      this.endPendingRun(r);
      return;
    }
    if (
      r.role === "parent" &&
      !pending.unreachableShown &&
      now - pending.startedAt >= PAIRING_UNREACHABLE_AFTER_MS
    ) {
      pending.unreachableShown = true;
      this.pairingStore.set(makePairingState("connecting", { error: "unreachable" }));
    }
  }

  // ---- state -------------------------------------------------------------------------------

  private setApp(patch: Partial<AppState>): void {
    this.appState = { ...this.appState, ...patch };
    this.appStore.set(this.appState);
    if (patch.monitoring !== undefined) this.page.setMonitoring(patch.monitoring);
  }

  private publishParent(): void {
    this.parentStore.set({ ...this.parentBase, ...this.parentTalk });
  }

  private publishBaby(): void {
    this.babyStore.set({ ...this.babyBase, ...this.babyTalk });
  }

  private updateSettingsState(patch: Partial<SettingsState>): void {
    this.settingsState = { ...this.settingsState, ...patch };
    this.settingsStore.set(this.settingsState);
  }
}

/** The user-entered TURN entry as the browser takes it; the credentials stay in this array. */
function userTurnServers(turn: TurnEntry | null): RTCIceServer[] {
  if (turn === null) return [];
  const entry: IceServer = { urls: [turn.url] };
  if (turn.username !== "") entry.username = turn.username;
  if (turn.password !== "") entry.credential = turn.password;
  return [toRtcIceServer(entry)];
}
