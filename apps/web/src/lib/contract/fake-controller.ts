import { writable, type Writable } from "svelte/store";
import type { AppController } from "./controller";
import { ScanError, type CodeScanner, type ScanFailure } from "./scanner";
import {
  defaultAppState,
  defaultBabyMonitor,
  defaultParentMonitor,
  defaultSettings,
  makePairingState,
  type AppState,
  type BabyMonitorState,
  type DeviceBattery,
  type PageGuardState,
  type PairingState,
  type ParentMonitorState,
  type SettingsState,
} from "./models";

/** A controller whose state tests and screenshots set directly, and whose calls are recorded. */
export interface FakeController extends AppController {
  readonly app: Writable<AppState>;
  readonly settings: Writable<SettingsState>;
  readonly parent: Writable<ParentMonitorState>;
  readonly baby: Writable<BabyMonitorState>;
  readonly pairing: Writable<PairingState | null>;
  readonly soundUnlocked: Writable<boolean>;
  readonly deviceBattery: Writable<DeviceBattery | null>;
  readonly pageGuard: Writable<PageGuardState>;
  /** Every action call as `[method, ...args]`, oldest first. */
  readonly calls: Array<[string, ...unknown[]]>;
}

/**
 * In-memory controller with the state transitions that need no platform: choosing a role,
 * starting and stopping monitoring, settings updates, snooze. Everything else only records the
 * call. The real controller (group 4) replaces it behind the same interface.
 */
export function createFakeController(): FakeController {
  const app = writable(defaultAppState());
  const settings = writable(defaultSettings());
  const parent = writable(defaultParentMonitor());
  const baby = writable(defaultBabyMonitor());
  const pairing = writable<PairingState | null>(null);
  const soundUnlocked = writable(false);
  const deviceBattery = writable<DeviceBattery | null>(null);
  const pageGuard = writable<PageGuardState>({ wakeLock: "off", hidden: false, away: null });
  const calls: Array<[string, ...unknown[]]> = [];
  const record =
    (name: string) =>
    (...args: unknown[]): void => {
      calls.push([name, ...args]);
    };

  return {
    app,
    settings,
    parent,
    baby,
    pairing,
    soundUnlocked,
    deviceBattery,
    pageGuard,
    calls,
    chooseRole(role) {
      calls.push(["chooseRole", role]);
      app.update((s) => ({ ...s, role }));
    },
    completeOnboarding() {
      calls.push(["completeOnboarding"]);
      app.update((s) => ({ ...s, onboardingDone: true }));
    },
    startPairing() {
      calls.push(["startPairing"]);
      pairing.set(makePairingState("showing_code"));
    },
    cancelPairing() {
      calls.push(["cancelPairing"]);
      pairing.set(null);
    },
    submitPairingCode: record("submitPairingCode"),
    refreshPairingCode: record("refreshPairingCode"),
    startMonitoring() {
      calls.push(["startMonitoring"]);
      app.update((s) => ({ ...s, monitoring: true }));
    },
    stopMonitoring() {
      calls.push(["stopMonitoring"]);
      app.update((s) => ({ ...s, monitoring: false }));
    },
    forgetPairing() {
      calls.push(["forgetPairing"]);
      app.update((s) => ({ ...s, paired: false, peerName: null, monitoring: false }));
    },
    updateSettings(transform) {
      calls.push(["updateSettings"]);
      settings.update(transform);
    },
    setListen(mode) {
      calls.push(["setListen", mode]);
      parent.update((s) => ({ ...s, listen: mode }));
    },
    requestTestAlert: record("requestTestAlert"),
    snooze(minutes) {
      calls.push(["snooze", minutes]);
      parent.update((s) => ({ ...s, snoozedUntilWallMs: Date.now() + minutes * 60_000 }));
    },
    cancelSnooze() {
      calls.push(["cancelSnooze"]);
      parent.update((s) => ({ ...s, snoozedUntilWallMs: null }));
    },
    acknowledgeAlarm: record("acknowledgeAlarm"),
    stopAlertSound() {
      calls.push(["stopAlertSound"]);
      parent.update((s) => ({ ...s, noiseAlertActive: false }));
    },
    requestSilenceBaby: record("requestSilenceBaby"),
    tryReconnectNow: record("tryReconnectNow"),
    dismissMissedEvents() {
      calls.push(["dismissMissedEvents"]);
      parent.update((s) => ({ ...s, missed: null }));
    },
    setKioskLocked(locked) {
      calls.push(["setKioskLocked", locked]);
      baby.update((s) => ({ ...s, kioskLocked: locked }));
    },
    dismissSilenceRestoredNotice() {
      calls.push(["dismissSilenceRestoredNotice"]);
      baby.update((s) => ({ ...s, silenceRestoredNotice: false }));
    },
    unlockSound() {
      calls.push(["unlockSound"]);
      soundUnlocked.set(true);
    },
    retryKeepAwake: record("retryKeepAwake"),
    dismissAwayReport() {
      calls.push(["dismissAwayReport"]);
      pageGuard.update((s) => ({ ...s, away: null }));
    },
    reportTestResult: record("reportTestResult"),
    dismissCoach() {
      calls.push(["dismissCoach"]);
      app.update((s) => ({ ...s, coachPending: false }));
    },
  };
}

/** A scanner whose camera tests drive: `emit` reads a code, `failNext` makes the next start fail. */
export interface FakeScanner extends CodeScanner {
  readonly started: HTMLVideoElement[];
  stops: number;
  emit(text: string): void;
  failNext(kind: ScanFailure): void;
}

export function createFakeScanner(): FakeScanner {
  let handler: ((text: string) => void) | null = null;
  let failure: ScanFailure | null = null;
  const fake: FakeScanner = {
    started: [],
    stops: 0,
    start(video, onCode) {
      if (failure !== null) {
        const kind = failure;
        failure = null;
        return Promise.reject(new ScanError(kind));
      }
      fake.started.push(video);
      handler = onCode;
      return Promise.resolve();
    },
    stop() {
      fake.stops++;
      handler = null;
    },
    emit(text) {
      const h = handler;
      handler = null;
      h?.(text);
    },
    failNext(kind) {
      failure = kind;
    },
  };
  return fake;
}
