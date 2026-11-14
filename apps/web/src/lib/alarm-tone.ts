/**
 * The connection alarm tone setting survives a reload in `localStorage`, next to the theme setting
 * (`theme.ts`). It is a parent preference, not pairing material, so the web-storage rule of
 * AGENTS.md section 7.2 does not apply. The raw string is stored; every read passes through the
 * core's `resolve_alarm_tone` (the controller does), so a garbage value is the default tone.
 */

export const ALARM_TONE_STORAGE_KEY = "guardedsleep.alarmTone";

/** Remembers the chosen tone id for the next visit; storage can be blocked (then this page only). */
export function storeAlarmToneSetting(storage: Pick<Storage, "setItem">, id: string): void {
  try {
    storage.setItem(ALARM_TONE_STORAGE_KEY, id);
  } catch {
    // Private window or site data off: nothing to do.
  }
}

/** The remembered tone id (unchecked: resolve it through the core), or null when none. */
export function readAlarmToneSetting(storage: Pick<Storage, "getItem">): string | null {
  try {
    return storage.getItem(ALARM_TONE_STORAGE_KEY);
  } catch {
    return null;
  }
}
