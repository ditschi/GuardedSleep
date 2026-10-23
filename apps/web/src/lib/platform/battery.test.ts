import { get } from "svelte/store";
import { describe, expect, it } from "vitest";
import { watchBattery, type BatteryManagerLike } from "./battery";

class FakeBattery implements BatteryManagerLike {
  level = 0.8;
  charging = true;
  private readonly listeners = new Map<string, Set<() => void>>();
  addEventListener(type: string, listener: () => void): void {
    this.listeners.set(type, (this.listeners.get(type) ?? new Set()).add(listener));
  }
  removeEventListener(type: string, listener: () => void): void {
    this.listeners.get(type)?.delete(listener);
  }
  fire(type: string): void {
    for (const l of this.listeners.get(type) ?? []) l();
  }
  count(type: string): number {
    return this.listeners.get(type)?.size ?? 0;
  }
}

const flush = (): Promise<void> => Promise.resolve().then(() => Promise.resolve());

describe("battery watcher", () => {
  it("reads the level and charging state, and follows their changes", async () => {
    const battery = new FakeBattery();
    const watcher = watchBattery({ getBattery: () => Promise.resolve(battery) });
    await flush();
    expect(get(watcher.state)).toEqual({ pct: 80, charging: true });
    expect(watcher.deviceStatus()).toEqual({ battery_pct: 80, charging: true });

    battery.level = 0.42;
    battery.charging = false;
    battery.fire("levelchange");
    expect(get(watcher.state)).toEqual({ pct: 42, charging: false });
    expect(watcher.deviceStatus()).toEqual({ battery_pct: 42, charging: false });
  });

  it("reports nothing where the browser has no Battery API", () => {
    const watcher = watchBattery({});
    expect(get(watcher.state)).toBeNull();
    expect(watcher.deviceStatus()).toEqual({});
  });

  it("reports nothing when the API refuses", async () => {
    const watcher = watchBattery({ getBattery: () => Promise.reject(new Error("blocked")) });
    await flush();
    expect(get(watcher.state)).toBeNull();
  });

  it("lets go of the listeners on stop", async () => {
    const battery = new FakeBattery();
    const watcher = watchBattery({ getBattery: () => Promise.resolve(battery) });
    await flush();
    watcher.stop();
    expect(battery.count("levelchange")).toBe(0);
    expect(battery.count("chargingchange")).toBe(0);
  });
});
