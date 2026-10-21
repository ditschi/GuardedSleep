import {
  HOLD_DURATION_CUSTOM_MAX_MS,
  HOLD_DURATION_CUSTOM_MIN_MS,
  HOLD_DURATION_DEFAULT_MS,
} from "./hold-duration-presets.js";

const KEY_THRESHOLD = "guardedsleep:baby:noise-threshold";
const KEY_HOLD_MS = "guardedsleep:baby:hold-ms";

export type BabyUnitPreferences = {
  threshold: number;
  holdMs: number;
};

export function readBabyUnitPreferences(): BabyUnitPreferences {
  const d: BabyUnitPreferences = {
    threshold: 0.035,
    holdMs: HOLD_DURATION_DEFAULT_MS,
  };
  if (typeof localStorage === "undefined") return d;
  try {
    const tr = Number.parseFloat(localStorage.getItem(KEY_THRESHOLD) ?? "");
    if (!Number.isNaN(tr) && tr >= 0.01 && tr <= 0.15) d.threshold = tr;
    const hm = Number.parseInt(localStorage.getItem(KEY_HOLD_MS) ?? "", 10);
    if (
      !Number.isNaN(hm) &&
      hm >= HOLD_DURATION_CUSTOM_MIN_MS &&
      hm <= HOLD_DURATION_CUSTOM_MAX_MS
    ) {
      d.holdMs = hm;
    }
  } catch {
    /* ignore corrupt storage */
  }
  return d;
}

export function persistBabyNoiseGate(threshold: number, holdMs: number): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(KEY_THRESHOLD, String(threshold));
  localStorage.setItem(KEY_HOLD_MS, String(holdMs));
}
