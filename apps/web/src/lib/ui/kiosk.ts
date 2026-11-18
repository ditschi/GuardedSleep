import type { DeviceBattery, LinkUi } from "../contract";
import type { MessageKey } from "../i18n/keys";
import { BABY_BATTERY_LOW_PCT, BATTERY_CRITICAL_PCT } from "./parent-view";

/*
 * The baby device's link mark and status line, mirrored from the Android kiosk (`KioskMark.kt`,
 * `connection-status` "Baby unit edge indicator", "Baby status line", "Baby needs-attention
 * mark"): the state is told by shape and words, never by hue alone.
 */

/** A short bar when fine, nothing at level 0, a dashed top edge at level 1, a pulsing frame from 2. */
export type KioskMarkShape = "none" | "bar" | "dashed" | "frame";

export function kioskMarkShape(link: LinkUi): KioskMarkShape {
  switch (link.phase) {
    case "connected":
      return "bar";
    // Nothing is wrong, or nobody is monitoring: the words say so, no mark.
    case "peer_stopped":
    case "idle":
    case "stopped":
      return "none";
    case "failed_needs_user":
      return "frame";
    case "connecting":
    case "reconnecting":
      if (link.level >= 2) return "frame";
      return link.level === 1 ? "dashed" : "none";
  }
}

/** Why the baby device needs a hand: its own battery or charger. */
export type KioskAttention = "not_charging" | "battery_low" | "battery_critical";

/**
 * `chargerSeen`: the charger was connected earlier in this run, so a reading that is not plugged
 * in now means it was pulled. A browser without the Battery API reports `null`: no mark.
 */
export function kioskAttention(
  battery: DeviceBattery | null,
  chargerSeen: boolean,
): KioskAttention | null {
  if (battery === null) return null;
  // Critical counts even while plugged in: a weak charger or a loose cable must not hide it.
  if (battery.pct < BATTERY_CRITICAL_PCT) return "battery_critical";
  if (battery.charging === true) return null;
  if (battery.pct < BABY_BATTERY_LOW_PCT) return "battery_low";
  return chargerSeen ? "not_charging" : null;
}

export const KIOSK_ATTENTION_KEY: Record<KioskAttention, MessageKey> = {
  not_charging: "native.ui.kiosk.attentionNotCharging",
  battery_low: "native.ui.kiosk.attentionBatteryLow",
  battery_critical: "native.ui.kiosk.attentionBatteryCritical",
};

/** The status line: "Listening" while connected, else the baby-side words for the state. */
export function kioskStateKey(link: LinkUi, statusKey: MessageKey): MessageKey {
  return link.phase === "connected" ? "native.ui.kiosk.listening" : statusKey;
}

/**
 * "State · Name" when the connection has a name (a label pattern, never a name inside a
 * sentence); the key-mismatch text is a whole sentence of its own and takes no name.
 */
export function kioskStatusLine(
  t: (key: MessageKey, args?: Record<string, string | number>) => string,
  link: LinkUi,
  statusKey: MessageKey,
  name: string | null,
): string {
  const state = t(kioskStateKey(link, statusKey));
  if (name === null || link.phase === "failed_needs_user") return state;
  return t("native.ui.kiosk.withName", { state, name });
}
