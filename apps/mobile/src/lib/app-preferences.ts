export type NightModePreference = "dark" | "light" | "system";
export type AlertAudioOutput = "default" | "speaker" | "speaker_default";

export type AppPreferences = {
  onboardingCompleted: boolean;
  showAdvancedSettings: boolean;
  showConnectionDetails: boolean;
  nightMode: NightModePreference;
  wakeLockInListenMode: boolean;
  listenHintSeen: boolean;
  alertAudioOutput: AlertAudioOutput;
  alertPlaybackGain: number;
  alertMinGainDuringAlert: boolean;
  heartbeatIntervalSec: number;
  probeRoutingHintSeen: boolean;
};

const STORAGE_KEY = "guardedsleep:app-preferences";

const HEARTBEAT_CHOICES = [5, 10, 15, 30] as const;

const DEFAULTS: AppPreferences = {
  onboardingCompleted: false,
  showAdvancedSettings: false,
  showConnectionDetails: false,
  nightMode: "system",
  wakeLockInListenMode: true,
  listenHintSeen: false,
  alertAudioOutput: "default",
  alertPlaybackGain: 1,
  alertMinGainDuringAlert: false,
  heartbeatIntervalSec: 10,
  probeRoutingHintSeen: false,
};

function clampHeartbeat(sec: number): number {
  return HEARTBEAT_CHOICES.includes(sec as (typeof HEARTBEAT_CHOICES)[number])
    ? sec
    : DEFAULTS.heartbeatIntervalSec;
}

function parseStored(raw: string | null): AppPreferences {
  if (raw == null || raw === "") return { ...DEFAULTS };
  try {
    const parsed = JSON.parse(raw) as Partial<AppPreferences>;
    return {
      onboardingCompleted: parsed.onboardingCompleted === true,
      showAdvancedSettings: parsed.showAdvancedSettings === true,
      showConnectionDetails: parsed.showConnectionDetails === true,
      nightMode:
        parsed.nightMode === "light" || parsed.nightMode === "dark" || parsed.nightMode === "system"
          ? parsed.nightMode
          : "system",
      wakeLockInListenMode: parsed.wakeLockInListenMode !== false,
      listenHintSeen: parsed.listenHintSeen === true,
      alertAudioOutput:
        parsed.alertAudioOutput === "speaker" || parsed.alertAudioOutput === "speaker_default"
          ? parsed.alertAudioOutput
          : "default",
      alertPlaybackGain:
        typeof parsed.alertPlaybackGain === "number" &&
        parsed.alertPlaybackGain >= 0.5 &&
        parsed.alertPlaybackGain <= 2
          ? parsed.alertPlaybackGain
          : DEFAULTS.alertPlaybackGain,
      alertMinGainDuringAlert: parsed.alertMinGainDuringAlert === true,
      heartbeatIntervalSec: clampHeartbeat(
        typeof parsed.heartbeatIntervalSec === "number" ? parsed.heartbeatIntervalSec : 10,
      ),
      probeRoutingHintSeen: parsed.probeRoutingHintSeen === true,
    };
  } catch {
    return { ...DEFAULTS };
  }
}

export function readAppPreferences(): AppPreferences {
  if (typeof localStorage === "undefined") return { ...DEFAULTS };
  return parseStored(localStorage.getItem(STORAGE_KEY));
}

export function persistAppPreferences(patch: Partial<AppPreferences>): AppPreferences {
  const next: AppPreferences = {
    ...readAppPreferences(),
    ...patch,
    heartbeatIntervalSec: clampHeartbeat(
      patch.heartbeatIntervalSec ?? readAppPreferences().heartbeatIntervalSec,
    ),
  };
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
  }
  return next;
}

export function heartbeatIntervalMs(): number {
  return readAppPreferences().heartbeatIntervalSec * 1000;
}

export const HEARTBEAT_INTERVAL_CHOICES = HEARTBEAT_CHOICES;

export const THEME_CHANGE_EVENT = "guardedsleep-theme-change";

export function resolveThemeDark(prefs: AppPreferences = readAppPreferences()): boolean {
  if (typeof window === "undefined") return prefs.nightMode === "dark";
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
  return prefs.nightMode === "dark" || (prefs.nightMode === "system" && prefersDark);
}

export function applyAppTheme(): void {
  if (typeof document === "undefined" || typeof window === "undefined") return;
  const dark = resolveThemeDark();
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
  const meta = document.querySelector('meta[name="theme-color"]');
  if (meta) meta.setAttribute("content", dark ? "#09090b" : "#f4f4f5");
  window.dispatchEvent(new CustomEvent(THEME_CHANGE_EVENT));
}
