/**
 * The sensitivity slider of the baby device. The core's noise threshold is a level (lower means
 * more sensitive); people think "how sensitive", so the slider runs the other way, 0..100.
 */
export const THRESHOLD_MIN = 0.03;
export const THRESHOLD_MAX = 0.4;

const clamp = (v: number, lo: number, hi: number): number => Math.min(hi, Math.max(lo, v));

/** Slider position 0 (least sensitive) to 100 (most sensitive) for a threshold. */
export function sensitivityOf(threshold: number): number {
  const span = THRESHOLD_MAX - THRESHOLD_MIN;
  return Math.round(clamp(((THRESHOLD_MAX - threshold) / span) * 100, 0, 100));
}

/** The threshold for a slider position, rounded to hundredths like the stored setting. */
export function thresholdOf(sensitivity: number): number {
  const span = THRESHOLD_MAX - THRESHOLD_MIN;
  const threshold = THRESHOLD_MAX - (clamp(sensitivity, 0, 100) / 100) * span;
  return Math.round(threshold * 100) / 100;
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
