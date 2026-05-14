/** Matches docs/features/audio-monitoring.md — duration presets and default. */
export const HOLD_DURATION_PRESET_MS = [3000, 5000, 10_000, 15_000] as const;

export const HOLD_DURATION_DEFAULT_MS = 5000;

/** Custom range: shorter than shortest preset down to 0.5s; longer for power users. */
export const HOLD_DURATION_CUSTOM_MIN_MS = 500;
export const HOLD_DURATION_CUSTOM_MAX_MS = 30_000;

export function formatHoldDurationLabel(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  const s = ms / 1000;
  return Number.isInteger(s) ? `${s} s` : `${s.toFixed(1)} s`;
}
