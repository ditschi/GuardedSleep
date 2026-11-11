import type { Readable } from "svelte/store";
import type {
  AppState,
  BabyMonitorState,
  BabyTalkUi,
  DeviceBattery,
  ListenMode,
  PageGuardState,
  PairingState,
  ParentMonitorState,
  ParentTalkUi,
  Role,
  SettingsState,
  TalkBackSettings,
} from "./models";
import type { TurnSaveResult } from "./turn";

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

  /** Web only: talk-back on the parent monitoring screen (`add-talk-back`). */
  readonly talk: Readable<ParentTalkUi>;
  /** Web only: talk-back on the baby device (indicator, last talk, settings, suspended audio). */
  readonly babyTalk: Readable<BabyTalkUi>;

  /** Sets the session role; `makeDefault` also stores it as the start mode. */
  chooseRole(role: Role, makeDefault?: boolean): void;
  /** Back to the role screen without changing the start mode; no-op while monitoring. */
  switchMode(): void;
  /** Sets the start mode: a role, or null for "Ask every time". */
  setDefaultRole(role: Role | null): void;
  completeOnboarding(): void;
  startPairing(): void;
  cancelPairing(): void;
  /** Parent: decoded QR text or pasted code. */
  submitPairingCode(payload: string): void;
  refreshPairingCode(): void;
  startMonitoring(): void;
  stopMonitoring(): void;
  forgetPairing(): void;
  /** `turnUrl`, `turnUsername` and `turnPasswordSaved` change only through `setTurnServer`. */
  updateSettings(transform: (settings: SettingsState) => SettingsState): void;
  /**
   * Web: memory only, used as an ICE server for every new attempt. An empty URL clears the entry;
   * an empty password keeps the saved one when URL and user name are unchanged.
   */
  setTurnServer(url: string, username: string, password: string): TurnSaveResult;
  clearTurnServer(): void;

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

  /**
   * Web only. The talk button went down (after its hold time). Without microphone permission this
   * opens the permission step and sends nothing.
   */
  talkPress(): void;
  /** Web only. The button was released or the touch was cancelled; also ends a talk in progress. */
  talkRelease(): void;
  /** Web only. The page left the front (hidden, blur): ends the talk and closes the microphone. */
  talkInterrupt(): void;
  /** Web only. "Allow microphone" in the permission step (a click handler: it may prompt). */
  requestTalkPermission(): void;
  closeTalkPermission(): void;

  // Baby
  setKioskLocked(locked: boolean): void;
  dismissSilenceRestoredNotice(): void;
  /** Web only. Talk-back settings of this baby device (switch, volume, chime); memory only. */
  setTalkBack(update: Partial<TalkBackSettings>): void;
  /** Web only. "Play chime": the chime at the chosen volume, to set the computer's volume by. */
  playTalkChimeSample(): void;
  /** Web only. The settings screen was opened by an adult (clears "may not be audible"). */
  talkSettingsOpened(): void;

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
