/**
 * Night mode (`ui-appearance` "Night mode", "Automatic night mode by time"): Off, On or Automatic
 * with a daily window, resolved against the device clock. The same rules as `NightSchedule` in
 * UiContract's `NightMode.kt`. `public/theme-boot.js` repeats `nightActive` in plain JavaScript so
 * the first paint already has the right scheme; `night-mode.test.ts` runs both and compares.
 * Presentation only: nothing here reaches the core, and no sensor is read.
 */

export type NightMode = "off" | "on" | "auto";

/** A manual change under Automatic: it wins until `untilWallMs`, the next window boundary. */
export interface NightOverride {
  active: boolean;
  untilWallMs: number;
}

/** The slice of `SettingsState` the window logic reads. */
export interface NightSettings {
  nightMode: NightMode;
  nightFromMinute: number;
  nightToMinute: number;
  nightOverride: NightOverride | null;
}

export const NIGHT_DEFAULT_FROM_MINUTE = 21 * 60;
export const NIGHT_DEFAULT_TO_MINUTE = 7 * 60;

const MS_PER_MINUTE = 60_000;
const MS_PER_DAY = 24 * 60 * MS_PER_MINUTE;

/** Night mode is a presentation preference, not secret; `theme-boot.js` reads this one key. */
export const NIGHT_STORAGE_KEY = "guardedsleep.night";

export function isNightMode(value: unknown): value is NightMode {
  return value === "off" || value === "on" || value === "auto";
}

/** A stored minute outside the day becomes the fallback. */
export function normalizeMinute(minute: unknown, fallback: number): number {
  return typeof minute === "number" && Number.isInteger(minute) && minute >= 0 && minute < 1440
    ? minute
    : fallback;
}

function floorMod(a: number, n: number): number {
  return ((a % n) + n) % n;
}

/** Offset of the local zone from UTC at an instant, in ms (what `Date` reports). */
export function localOffsetMs(wallMs: number): number {
  return -new Date(wallMs).getTimezoneOffset() * MS_PER_MINUTE;
}

/**
 * Whether `minuteOfDay` lies in the window `from` (inclusive) to `to` (exclusive). A window that
 * ends before it starts wraps past midnight; equal ends make an empty window, never on.
 */
export function inWindow(from: number, to: number, minuteOfDay: number): boolean {
  if (from === to) return false;
  if (from < to) return minuteOfDay >= from && minuteOfDay < to;
  return minuteOfDay >= from || minuteOfDay < to;
}

/** The first window start or end strictly after `nowWallMs`, or null for an empty window. */
export function nextBoundaryWallMs(
  from: number,
  to: number,
  nowWallMs: number,
  offsetMs: number,
): number | null {
  if (from === to) return null;
  const now = floorMod(nowWallMs + offsetMs, MS_PER_DAY);
  const until = (minute: number): number => {
    const d = floorMod(minute * MS_PER_MINUTE - now, MS_PER_DAY);
    return d === 0 ? MS_PER_DAY : d;
  };
  return nowWallMs + Math.min(until(from), until(to));
}

/** Whether night mode is on right now. */
export function nightActive(s: NightSettings, nowWallMs: number, offsetMs: number): boolean {
  if (s.nightMode === "off") return false;
  if (s.nightMode === "on") return true;
  const o = s.nightOverride;
  // An override further away than a day was written under another clock.
  if (o !== null && nowWallMs < o.untilWallMs && o.untilWallMs - nowWallMs <= MS_PER_DAY) {
    return o.active;
  }
  const minute = Math.floor(floorMod(nowWallMs + offsetMs, MS_PER_DAY) / MS_PER_MINUTE);
  return inWindow(s.nightFromMinute, s.nightToMinute, minute);
}

/**
 * The settings after the night switch is tapped: off while on and on while off. Under Automatic
 * the choice becomes an override until the next window boundary; otherwise the mode itself flips.
 */
export function toggledNight<T extends NightSettings>(
  s: T,
  nowWallMs: number,
  offsetMs: number,
): T {
  const target = !nightActive(s, nowWallMs, offsetMs);
  if (s.nightMode === "auto") {
    const until = nextBoundaryWallMs(s.nightFromMinute, s.nightToMinute, nowWallMs, offsetMs);
    // An empty window has no boundary to hold an override until: the choice becomes the mode.
    if (until === null) return { ...s, nightMode: target ? "on" : "off", nightOverride: null };
    return { ...s, nightOverride: { active: target, untilWallMs: until } };
  }
  return { ...s, nightMode: target ? "on" : "off", nightOverride: null };
}

/** The settings after the user picks a mode: an override never survives. */
export function withNightMode<T extends NightSettings>(s: T, nightMode: NightMode): T {
  return { ...s, nightMode, nightOverride: null };
}

/** The settings after the window changes: the old boundary no longer applies. */
export function withNightWindow<T extends NightSettings>(s: T, from: number, to: number): T {
  return { ...s, nightFromMinute: from, nightToMinute: to, nightOverride: null };
}

/** `HH:MM` for `<input type="time">`. */
export function minuteToTimeValue(minute: number): string {
  const h = Math.floor(minute / 60);
  const m = minute % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

/** Minutes since midnight from an `<input type="time">` value, or null for an empty or bad one. */
export function timeValueToMinute(value: string): number | null {
  const match = /^(\d{2}):(\d{2})/.exec(value);
  if (match === null) return null;
  const h = Number(match[1]);
  const m = Number(match[2]);
  return h < 24 && m < 60 ? h * 60 + m : null;
}

/** The remembered night settings, or null when none (or garbage) is stored. */
export function readNightSettings(storage: Pick<Storage, "getItem">): NightSettings | null {
  try {
    const raw = storage.getItem(NIGHT_STORAGE_KEY);
    if (raw === null) return null;
    return parseNightSettings(JSON.parse(raw) as unknown);
  } catch {
    return null;
  }
}

/** Validates a parsed stored value; unknown parts fall back to the defaults. */
export function parseNightSettings(value: unknown): NightSettings | null {
  if (typeof value !== "object" || value === null) return null;
  const v = value as Record<string, unknown>;
  if (!isNightMode(v.mode)) return null;
  const o = v.override;
  let override: NightOverride | null = null;
  if (typeof o === "object" && o !== null) {
    const r = o as Record<string, unknown>;
    if (typeof r.active === "boolean" && typeof r.until === "number" && Number.isFinite(r.until)) {
      override = { active: r.active, untilWallMs: r.until };
    }
  }
  return {
    nightMode: v.mode,
    nightFromMinute: normalizeMinute(v.from, NIGHT_DEFAULT_FROM_MINUTE),
    nightToMinute: normalizeMinute(v.to, NIGHT_DEFAULT_TO_MINUTE),
    nightOverride: override,
  };
}

/**
 * Remembers the night settings for the next visit: `theme-boot.js` reads them before first paint.
 * Storage can be blocked; the choice then lasts for this page only.
 */
export function storeNightSettings(storage: Pick<Storage, "setItem">, s: NightSettings): void {
  try {
    storage.setItem(
      NIGHT_STORAGE_KEY,
      JSON.stringify({
        mode: s.nightMode,
        from: s.nightFromMinute,
        to: s.nightToMinute,
        override:
          s.nightOverride === null
            ? null
            : { active: s.nightOverride.active, until: s.nightOverride.untilWallMs },
      }),
    );
  } catch {
    // Private window or site data off: nothing to do.
  }
}

function sameNight(a: NightSettings, b: NightSettings): boolean {
  return (
    a.nightMode === b.nightMode &&
    a.nightFromMinute === b.nightFromMinute &&
    a.nightToMinute === b.nightToMinute &&
    a.nightOverride?.active === b.nightOverride?.active &&
    a.nightOverride?.untilWallMs === b.nightOverride?.untilWallMs
  );
}

/**
 * Writes the night settings when they differ from what is stored, so a default install stores
 * nothing until the user changes something. Called on every settings change (Display page, the
 * monitor's night switch), so no screen persists the choice on its own.
 */
export function persistNightSettings(
  storage: Pick<Storage, "getItem" | "setItem">,
  s: NightSettings,
): void {
  const current: NightSettings = {
    nightMode: s.nightMode,
    nightFromMinute: s.nightFromMinute,
    nightToMinute: s.nightToMinute,
    nightOverride: s.nightOverride,
  };
  const stored = readNightSettings(storage);
  const baseline: NightSettings = stored ?? {
    nightMode: "off",
    nightFromMinute: NIGHT_DEFAULT_FROM_MINUTE,
    nightToMinute: NIGHT_DEFAULT_TO_MINUTE,
    nightOverride: null,
  };
  if (!sameNight(baseline, current)) storeNightSettings(storage, current);
}
