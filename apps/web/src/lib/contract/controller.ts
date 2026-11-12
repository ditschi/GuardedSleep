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
  WebTalkState,
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

  /**
   * Web only: what the browser adds to talk-back (no microphone support here, audio suspended on
   * the baby device); the talk models themselves are in `parent` and `baby`.
   */
  readonly webTalk: Readable<WebTalkState>;

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

  // Talk-back (`add-talk-back`), the same intents as UiContract.kt.
  /**
   * Parent: the talk button went down (after its hold time) or up; also ends a talk when the
   * touch is cancelled. Without microphone permission a press opens the permission step and
   * sends nothing.
   */
  setTalk(pressed: boolean): void;
  /** Parent: the page left the front (hidden, blur): ends the talk and closes the microphone. */
  interruptTalk(): void;
  /** Parent: the answer of the browser's microphone prompt. */
  talkPermissionResult(granted: boolean): void;
  /**
   * Web only: "Allow microphone" in the permission step. The browser's prompt is page I/O, so the
   * controller runs it and reports through `talkPermissionResult`; on Android the screen hosts the
   * system dialog and calls `talkPermissionResult` itself.
   */
  requestTalkPermission(): void;
  dismissTalkPermission(): void;
  dismissTalkCoach(): void;

  // Baby
  setKioskLocked(locked: boolean): void;
  dismissSilenceRestoredNotice(): void;
  /** Baby: "Play chime" in the settings, at the chosen level. */
  playTalkChimeSample(): void;
  /** Baby: an adult opened the talk-back settings (ends the "may not be audible" warning). */
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
