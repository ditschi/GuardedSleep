import type { DeviceStatus } from "@guardedsleep/core-wasm";
import { writable, type Readable } from "svelte/store";
import type { DeviceBattery } from "../contract";

/** The Battery Status API (Chromium); Firefox and Safari do not have it. */
export interface BatteryManagerLike {
  level: number;
  charging: boolean;
  addEventListener(type: "levelchange" | "chargingchange", listener: () => void): void;
  removeEventListener(type: "levelchange" | "chargingchange", listener: () => void): void;
}

export interface BatteryWatcher {
  readonly state: Readable<DeviceBattery | null>;
  /** The baby's device status for the parent; empty where the browser reports no battery. */
  deviceStatus(): DeviceStatus;
  stop(): void;
}

/**
 * Reads the battery where the browser allows it and keeps `state` current. A browser reports no
 * ringer or Do Not Disturb, so the status carries the battery only (that is also what marks the
 * baby device as a web client for the parent).
 */
export function watchBattery(nav: {
  getBattery?: () => Promise<BatteryManagerLike>;
}): BatteryWatcher {
  const store = writable<DeviceBattery | null>(null);
  let current: DeviceBattery | null = null;
  let manager: BatteryManagerLike | null = null;
  let stopped = false;

  const read = (): void => {
    if (!manager) return;
    current = { pct: Math.round(manager.level * 100), charging: manager.charging };
    store.set(current);
  };
  if (typeof nav.getBattery === "function") {
    nav.getBattery().then(
      (m) => {
        if (stopped) return;
        manager = m;
        m.addEventListener("levelchange", read);
        m.addEventListener("chargingchange", read);
        read();
      },
      () => undefined,
    );
  }

  return {
    state: { subscribe: store.subscribe },
    deviceStatus() {
      if (current === null) return {};
      return {
        battery_pct: current.pct,
        ...(current.charging === null ? {} : { charging: current.charging }),
      };
    },
    stop() {
      stopped = true;
      manager?.removeEventListener("levelchange", read);
      manager?.removeEventListener("chargingchange", read);
    },
  };
}
