import type { DeviceStatus, UnitWarning } from "@guardedsleep/core-wasm";

type BatteryManager = EventTarget & { level: number; charging: boolean };

/** Battery percent as sent on the wire: steps of 5 (ADR-018 privacy budget). */
export function bucketBattery(level: number): number {
  return Math.min(100, Math.max(0, Math.round((level * 100) / 5) * 5));
}

/**
 * A browser baby knows only its battery (where `navigator.getBattery()` exists); ringer, Do Not
 * Disturb and alarms stay unknown, which the parent shows as "can't check".
 */
export function watchBrowserDeviceStatus(onStatus: (status: DeviceStatus) => void): () => void {
  const nav = navigator as Navigator & { getBattery?: () => Promise<BatteryManager> };
  if (typeof nav.getBattery !== "function") {
    onStatus({});
    return () => {};
  }
  let off = false;
  let battery: BatteryManager | null = null;
  const publish = (): void => {
    if (off || !battery) return;
    onStatus({ battery_pct: bucketBattery(battery.level), charging: battery.charging });
  };
  nav
    .getBattery()
    .then((b) => {
      battery = b;
      b.addEventListener("levelchange", publish);
      b.addEventListener("chargingchange", publish);
      publish();
    })
    .catch(() => onStatus({}));
  return () => {
    off = true;
    battery?.removeEventListener("levelchange", publish);
    battery?.removeEventListener("chargingchange", publish);
  };
}

/** `resumed_from_sleep` stays reported this long after a suspend gap was detected. */
export const RESUMED_WARNING_MS = 10 * 60_000;

export type UnitSignals = {
  pageHidden: boolean;
  wakeLockHeld: boolean;
  /** Wall time of the last detected suspend/resume gap; null if none happened. */
  lastGapWallMs: number | null;
  nowWallMs: number;
  /** Battery API: false = running on battery; null = unknown. */
  charging: boolean | null;
  micEnded: boolean;
};

/** What could make this browser baby stop watching, for the parent's readiness chip. */
export function computeUnitWarnings(sig: UnitSignals): UnitWarning[] {
  const out: UnitWarning[] = [];
  if (sig.pageHidden) out.push("page_hidden");
  if (!sig.wakeLockHeld) out.push("wake_lock_off");
  if (sig.lastGapWallMs != null && sig.nowWallMs - sig.lastGapWallMs < RESUMED_WARNING_MS) {
    out.push("resumed_from_sleep");
  }
  if (sig.charging === false) out.push("not_charging");
  if (sig.micEnded) out.push("mic_permission_lost");
  return out;
}

/** Battery fields plus warnings, in the shape the core sends to the parent. */
export function mergeBabyDeviceStatus(
  battery: DeviceStatus,
  warnings: UnitWarning[],
): DeviceStatus {
  const merged: DeviceStatus = { ...battery };
  delete merged.unit_warnings;
  if (warnings.length > 0) merged.unit_warnings = warnings;
  return merged;
}
