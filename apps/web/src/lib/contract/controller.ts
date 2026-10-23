import type { Readable } from "svelte/store";
import type {
  AppState,
  BabyMonitorState,
  DeviceBattery,
  ListenMode,
  PageGuardState,
  PairingState,
  ParentMonitorState,
  Role,
  SettingsState,
} from "./models";

/**
 * What the screens read and call; the platform layer implements it. The state is a set of stores
 * (the web counterpart of the `StateFlow`s in `AppController` of UiContract.kt); the actions are
 * the same methods. The UI imports this interface and the models, nothing from the core bindings.
 */
export interface AppController {
  readonly app: Readable<AppState>;
  readonly settings: Readable<SettingsState>;
  readonly parent: Readable<ParentMonitorState>;
  readonly baby: Readable<BabyMonitorState>;
  readonly pairing: Readable<PairingState | null>;
  /**
   * Web only (no counterpart in UiContract.kt): browsers keep audio locked until a user gesture.
   * False until `unlockSound()` ran from one; alerts and baby audio are silent before.
   */
  readonly soundUnlocked: Readable<boolean>;
  /**
   * Web only: this device's battery from the Battery API, null where the browser has none
   * (Firefox, Safari). The baby device also reports it to the parent.
   */
  readonly deviceBattery: Readable<DeviceBattery | null>;
  /**
   * Web only: wake lock, page visibility and away reports while monitoring (a browser can pause
   * or freeze a page that is not in front; the screens warn about it).
   */
  readonly pageGuard: Readable<PageGuardState>;

  chooseRole(role: Role): void;
  completeOnboarding(): void;
  startPairing(): void;
  cancelPairing(): void;
  /** Parent: decoded QR text or pasted code. */
  submitPairingCode(payload: string): void;
  refreshPairingCode(): void;
  startMonitoring(): void;
  stopMonitoring(): void;
  forgetPairing(): void;
  updateSettings(transform: (settings: SettingsState) => SettingsState): void;

  // Parent
  setListen(mode: ListenMode): void;
  requestTestAlert(): void;
  snooze(minutes: number): void;
  cancelSnooze(): void;
  acknowledgeAlarm(): void;
  stopAlertSound(): void;
  requestSilenceBaby(): void;
  tryReconnectNow(): void;
  dismissMissedEvents(): void;

  // Baby
  setKioskLocked(locked: boolean): void;
  dismissSilenceRestoredNotice(): void;

  /** Web only. Call it from a click handler ("Tap to turn on alert sound"); it unlocks audio. */
  unlockSound(): void;

  /** Web only. Takes the screen wake lock again after the system released it. */
  retryKeepAwake(): void;
  /** Web only. Clears the "page was in the background / device slept" report. */
  dismissAwayReport(): void;

  /** Parent answered "Did you hear and feel it?" after a test alert. */
  reportTestResult(heardAndFelt: boolean): void;
  dismissCoach(): void;
}
