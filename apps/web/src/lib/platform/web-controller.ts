import type {
  CoreApi,
  DeviceStatus,
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
  defaultSettings,
  makePairingState,
  type AppController,
  type AppState,
  type BabyMonitorState,
  type DeviceBattery,
  type ListenMode,
  type PageGuardState,
  type PairingError,
  type PairingState,
  type ParentMonitorState,
  type Role,
  type SettingsState,
  type TurnSaveResult,
} from "../contract";
import { resolveTurnSave, turnSettings, type TurnEntry } from "../contract/turn";
import type { AlertSink } from "./alert-sink";
import type { SoundGate } from "./audio";
import { BabyModel } from "./baby-model";
import type { Clock, Timers } from "./clock";
import { encodeBase64Url } from "./encoding";
import { mailboxesForUrls, signalingUrlsOf } from "./invite-mailboxes";
import { LinkDriver, type LinkListener } from "./link-driver";
import type { InboundAudioStats, MediaEngine } from "./media-engine";
import { idleWatch, type PageWatch } from "./page-watch";
import { ParentModel } from "./parent-model";
import {
  ALARM_AFTER_MS,
  INVITE_VALIDITY_MS,
  MAX_HEARTBEAT_MS,
  MIN_HEARTBEAT_MS,
  PAIRING_UNREACHABLE_AFTER_MS,
  PSK_BYTES,
} from "./policy";
import { iceServersFor } from "./relay-stun";
import type { SignalingFactory } from "./signaling";

/** Tells the controller when the network came back (the core then retries at once). */
export interface NetworkWatcher {
  start(onAvailable: () => void): void;
  stop(): void;
}

export interface ControllerDeps {
  core: Pick<CoreApi, "createLink" | "buildInvite" | "parseInvite">;
  clock: Clock;
  timers: Timers;
  signaling: SignalingFactory;
  /** Opens the microphone (baby) or builds the playout chain (parent); rejects on a refusal. */
  createMedia(role: Role, iceServers: RTCIceServer[]): Promise<MediaEngine>;
  gate: SoundGate;
  alerts: AlertSink;
  network: NetworkWatcher;
  randomBytes(length: number): Uint8Array;
  /** 53 random bits for the core's jitter. */
  entropy(): number;
  /** Where to connect when neither the settings nor an invite name a relay. */
  defaultSignalingUrls(): string[];
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
  /** Wake lock, page visibility and sleep detection; absent in tests that do not need them. */
  page?: PageWatch;
}

/** Pairing material of this page. In memory only: never storage, URLs, logs or the DOM. */
interface StoredPairing {
  pskB64Url: string;
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

  readonly app: Readable<AppState> = this.appStore;
  readonly settings: Readable<SettingsState> = this.settingsStore;
  readonly pairing: Readable<PairingState | null> = this.pairingStore;
  readonly soundUnlocked: Readable<boolean> = this.soundStore;
  readonly parent: Readable<ParentMonitorState> = this.parentStore;
  readonly baby: Readable<BabyMonitorState> = this.babyStore;
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
    this.parentModel = new ParentModel(
      deps.clock,
      deps.alerts,
      {
        muteRemote: (muted) => this.run?.driver?.media((m) => m.setPlaybackMuted(muted)),
        listenStopped: () => this.run?.driver?.input({ type: "set_listen", enabled: false }),
        testAlertArrived: (wallMs) => this.updateSettingsState({ lastTestWallMs: wallMs }),
        snoozeEnded: () => undefined,
      },
      () => this.settingsState.threshold,
      (state) => this.parentStore.set(state),
    );
    this.babyModel = new BabyModel(deps.clock, (state) => this.babyStore.set(state));
    this.babyModel.setSettings(this.settingsState.threshold, this.settingsState.holdSeconds);
    deps.gate.onChange((unlocked) => this.soundStore.set(unlocked));
  }

  // ---- app ---------------------------------------------------------------------------------

  chooseRole(role: Role): void {
    if (this.appState.role !== null && this.appState.role !== role) this.forgetPairing();
    this.setApp({ role });
  }

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
    const urls = this.effectiveUrls(null);
    const now = this.deps.clock.elapsedMs();
    const psk = this.deps.randomBytes(PSK_BYTES);
    const expiresAtS = Math.floor(this.deps.clock.wallMs() / 1000) + INVITE_VALIDITY_MS / 1000;
    const invite = urls.length
      ? this.deps.core.buildInvite(psk, mailboxesForUrls(urls), expiresAtS)
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
      r.driver.input({ type: "set_listen", enabled: mode !== "off" });
    }
  }

  requestTestAlert(): void {
    const driver = this.run?.driver;
    if (driver && this.parentModel.state.link.phase === "connected") {
      driver.input({ type: "request_probe" });
    } else {
      // Tested this device only: the chain runs locally with the baby device not involved.
      this.deps.alerts.noiseAlert("new", true, false);
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

    const signalingUrls = this.effectiveUrls(pending.urls);
    let media: MediaEngine;
    try {
      media = await this.deps.createMedia(role, iceServersFor(signalingUrls, { turn: this.turn }));
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
      deviceStatus: role === "baby" ? (this.deps.deviceStatus ?? (() => ({}))) : undefined,
      // A browser cannot silence its device; the parent then sees "unknown", never a promise.
      applyQuiet: role === "baby" ? (this.deps.deviceStatus ?? (() => ({}))) : undefined,
    });
    run.driver = driver;
    this.deps.network.start(() => driver.input({ type: "network_available" }));
    driver.start();
    if (role === "parent") {
      driver.media((m) => m.setPlaybackGainPct(settings.alertGainPct));
      const snoozeUntil = this.parentModel.snoozeUntilElapsedMs();
      if (snoozeUntil !== null) driver.input({ type: "set_snooze", until_ms: snoozeUntil });
    }
  }

  private onMediaFailed(run: Run, error: unknown): void {
    this.run = null;
    this.deps.alerts.clear();
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
    this.deps.alerts.clear();
  }

  private endPendingRun(r: Run): void {
    if (this.run !== r) return;
    this.run = null;
    this.deps.network.stop();
    r.driver?.stop();
    this.parentModel.reset();
    this.babyModel.reset();
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
      // The relay mailboxes of the pairing: the core opens each at its own address. The baby also
      // sends the set to the parent after each verification.
      mailboxes: mailboxesForUrls(this.effectiveUrls(pending.urls)),
      allow_dev_loopback: this.deps.allowDevLoopback ?? false,
    };
  }

  /** The advanced list overrides the invite's list; otherwise the invite's, then the settings'. */
  private effectiveUrls(inviteUrls: string[] | null): string[] {
    const s = this.settingsState;
    if (s.advancedEnabled && s.signalingUrls.length > 0) return s.signalingUrls;
    if (inviteUrls && inviteUrls.length > 0) return inviteUrls;
    return s.signalingUrls.length > 0 ? s.signalingUrls : this.deps.defaultSignalingUrls();
  }

  // ---- core callbacks ----------------------------------------------------------------------

  private listenerFor(r: Run): LinkListener {
    const current = (): boolean => this.run === r;
    return {
      onStatus: (status: LinkStatus, now: number) => {
        if (!current()) return;
        if (r.role === "parent") this.parentModel.onStatus(status, now);
        else this.babyModel.onStatus(status);
        this.onPairingStatus(r, status);
      },
      onEvent: (event: LinkEvent, now: number) => {
        if (!current()) return;
        // The one event that carries the parent secret: kept in memory next to the PSK, never logged.
        if (event.type === "parent_pinned")
          this.keepParentSecret(r, event.secret, event.window_until_s ?? null);
        else if (event.type === "mailboxes_updated") this.keepMailboxes(r, event.mailboxes);
        if (r.role === "parent") this.parentModel.onEvent(event, now);
        else this.babyModel.onEvent(event);
      },
      onTick: (now: number) => {
        if (!current()) return;
        if (r.role === "parent") this.parentModel.onTick(now);
        this.checkPairingTimers(r, now);
      },
      onMicLevel: (rms: number, now: number) => {
        if (current()) this.babyModel.onMicLevel(rms, now);
      },
      onRemoteLevel: (rms: number, now: number) => {
        if (current()) this.parentModel.onRemoteLevel(rms, now);
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

  private updateSettingsState(patch: Partial<SettingsState>): void {
    this.settingsState = { ...this.settingsState, ...patch };
    this.settingsStore.set(this.settingsState);
  }
}
