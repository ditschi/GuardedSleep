export const METER_BARS = 8;

export interface MeterBar {
  on: boolean;
  zone: "low" | "mid" | "high";
}

/** Maps a 0..1 level to the lit bars: first 5 low, then 2 mid, the last one high. */
export function meterBars(level: number): MeterBar[] {
  const clamped = Number.isFinite(level) ? Math.min(1, Math.max(0, level)) : 0;
  const lit = Math.round(clamped * METER_BARS);
  return Array.from({ length: METER_BARS }, (_, i) => ({
    on: i < lit,
    zone: i < 5 ? "low" : i < 7 ? "mid" : "high",
  }));
}
