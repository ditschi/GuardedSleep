import { writable, type Writable } from "svelte/store";
import type { AppController } from "./controller";
import {
  defaultAppState,
  defaultBabyMonitor,
  defaultParentMonitor,
  defaultSettings,
  makePairingState,
  type AppState,
  type BabyMonitorState,
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
    reportTestResult: record("reportTestResult"),
    dismissCoach() {
      calls.push(["dismissCoach"]);
      app.update((s) => ({ ...s, coachPending: false }));
    },
  };
}
