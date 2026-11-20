/**
 * The sensitivity slider of the baby device, in decibels (design section 3). The core's noise
 * threshold is a linear level (lower means more sensitive); people think "how sensitive", so the
 * slider runs the other way, 0 (least) to 100 (most), in even steps of loudness:
 * `dBFS = -55 + 40 * (1 - p)`, threshold `10^(dBFS / 20)`. The most sensitive end stays above the
 * core's lowest accepted threshold (`minNoiseThreshold()`, applied by the controller), so no slider
 * position is ever refused on the wire.
 */
export const MOST_SENSITIVE_DBFS = -55;
export const SENSITIVITY_SPAN_DB = 40;

const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));

/** The level in dBFS for a slider position 0..100. */
export function dbfsOf(sensitivity: number): number {
  return MOST_SENSITIVE_DBFS + SENSITIVITY_SPAN_DB * (1 - clamp(sensitivity, 0, 100) / 100);
}

/** Slider position 0 (least sensitive) to 100 (most sensitive) for a threshold. */
export function sensitivityOf(threshold: number): number {
  const db = 20 * Math.log10(Math.max(threshold, 1e-9));
  const p = 1 - (db - MOST_SENSITIVE_DBFS) / SENSITIVITY_SPAN_DB;
  return Math.round(clamp(p, 0, 1) * 100);
}

/** The threshold for a slider position, rounded to 4 decimals like the core's status field. */
export function thresholdOf(sensitivity: number): number {
  const threshold = 10 ** (dbfsOf(sensitivity) / 20);
  return Math.round(threshold * 10_000) / 10_000;
}

/** `ws://` or `wss://` with a host: the only addresses the signaling transport can open. */
export function isSignalingUrl(text: string): boolean {
  try {
    const url = new URL(text.trim());
    return (url.protocol === "ws:" || url.protocol === "wss:") && url.hostname !== "";
  } catch {
    return false;
  }
}
