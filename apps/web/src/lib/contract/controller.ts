import type { Readable } from "svelte/store";
import type {
  AppState,
  BabyMonitorState,
  ListenMode,
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

  /** Parent answered "Did you hear and feel it?" after a test alert. */
  reportTestResult(heardAndFelt: boolean): void;
  dismissCoach(): void;
}
