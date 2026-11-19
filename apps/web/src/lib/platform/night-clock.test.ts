import { get, writable } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { NightSettings } from "../night-mode";
import { createNightClock } from "./night-clock";

const MIN = 60_000;
const DAY = 24 * 60 * MIN;
const at = (day: number, hour: number, minute = 0, second = 0): number =>
  day * DAY + (hour * 60 + minute) * MIN + second * 1000;

const auto: NightSettings = {
  nightMode: "auto",
  nightFromMinute: 21 * 60,
  nightToMinute: 7 * 60,
  nightOverride: null,
};

describe("createNightClock", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("switches at the window boundary", () => {
    vi.setSystemTime(at(5, 20, 59, 30));
    const clock = createNightClock(writable(auto), Date.now, () => 0);
    const seen: boolean[] = [];
    const stop = clock.subscribe((v) => seen.push(v));
    expect(seen).toEqual([false]);
    vi.advanceTimersByTime(29_000);
    expect(seen).toEqual([false]);
    vi.advanceTimersByTime(1_500);
    expect(seen).toEqual([false, true]);
    stop();
  });

  it("follows the setting and ends an override at its boundary", () => {
    vi.setSystemTime(at(5, 23));
    const settings = writable<NightSettings>({ ...auto, nightMode: "off" });
    const clock = createNightClock(settings, Date.now, () => 0);
    const seen: boolean[] = [];
    const stop = clock.subscribe((v) => seen.push(v));
    settings.set({ ...auto, nightMode: "on" });
    // Automatic, but switched off until 07:00.
    settings.set({ ...auto, nightOverride: { active: false, untilWallMs: at(6, 7) } });
    expect(seen).toEqual([false, true, false]);
    vi.advanceTimersByTime(8 * 60 * MIN + 1_000);
    expect(seen).toEqual([false, true, false]);
    vi.advanceTimersByTime(14 * 60 * MIN);
    expect(get(clock)).toBe(true);
    stop();
  });

  it("starts no timer outside Automatic and stops its timer on unsubscribe", () => {
    vi.setSystemTime(at(5, 12));
    const clock = createNightClock(writable({ ...auto, nightMode: "on" }), Date.now, () => 0);
    const stop = clock.subscribe(() => undefined);
    expect(vi.getTimerCount()).toBe(0);
    stop();
    const autoClock = createNightClock(writable(auto), Date.now, () => 0);
    const stopAuto = autoClock.subscribe(() => undefined);
    expect(vi.getTimerCount()).toBe(1);
    stopAuto();
    expect(vi.getTimerCount()).toBe(0);
  });
});
