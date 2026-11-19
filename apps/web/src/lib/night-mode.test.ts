import { describe, expect, it } from "vitest";
import {
  NIGHT_STORAGE_KEY,
  inWindow,
  minuteToTimeValue,
  nextBoundaryWallMs,
  nightActive,
  normalizeMinute,
  parseNightSettings,
  persistNightSettings,
  readNightSettings,
  storeNightSettings,
  timeValueToMinute,
  toggledNight,
  withNightMode,
  withNightWindow,
  type NightSettings,
} from "./night-mode";

const MIN = 60_000;
const DAY = 24 * 60 * MIN;
const FROM = 21 * 60;
const TO = 7 * 60;

/** Local clock time on day `day` (UTC, offset 0). */
const at = (day: number, hour: number, minute = 0, second = 0): number =>
  day * DAY + (hour * 60 + minute) * MIN + second * 1000;

const base: NightSettings = {
  nightMode: "off",
  nightFromMinute: FROM,
  nightToMinute: TO,
  nightOverride: null,
};
const auto = (nightOverride: NightSettings["nightOverride"] = null): NightSettings => ({
  ...base,
  nightMode: "auto",
  nightOverride,
});

describe("the window", () => {
  it("wraps past midnight when it ends before it starts", () => {
    const on = (h: number, m = 0): boolean => inWindow(FROM, TO, h * 60 + m);
    expect([on(21), on(23, 59), on(0), on(3), on(6, 59)]).toEqual([true, true, true, true, true]);
    expect([on(7), on(12), on(20, 59)]).toEqual([false, false, false]);
  });

  it("does not wrap inside one day, and an empty window is never on", () => {
    expect(inWindow(13 * 60, 15 * 60, 14 * 60)).toBe(true);
    expect(inWindow(13 * 60, 15 * 60, 15 * 60)).toBe(false);
    expect(inWindow(8 * 60, 8 * 60, 8 * 60)).toBe(false);
    expect(nextBoundaryWallMs(8 * 60, 8 * 60, at(3, 8), 0)).toBeNull();
  });

  it("finds the next start or end strictly after now", () => {
    const next = (now: number): number | null => nextBoundaryWallMs(FROM, TO, now, 0);
    expect(next(at(5, 12))).toBe(at(5, 21));
    expect(next(at(5, 22))).toBe(at(6, 7));
    expect(next(at(5, 3))).toBe(at(5, 7));
    expect(next(at(5, 7))).toBe(at(5, 21));
    expect(next(at(5, 21))).toBe(at(6, 7));
    expect(next(at(5, 20, 59, 59))).toBe(at(5, 21));
  });

  it("follows the local zone offset", () => {
    const plus2 = 2 * 60 * MIN;
    expect(nightActive(auto(), at(5, 19), plus2)).toBe(true);
    expect(nightActive(auto(), at(5, 18, 59), plus2)).toBe(false);
    expect(nextBoundaryWallMs(FROM, TO, at(5, 12), plus2)).toBe(at(5, 19));
    expect(nightActive(auto(), -3 * 60 * MIN, 0)).toBe(true);
  });

  it("is off and on whatever the clock says, outside Automatic", () => {
    const held = { active: true, untilWallMs: at(5, 18) };
    expect(nightActive({ ...base, nightOverride: held }, at(5, 22), 0)).toBe(false);
    expect(nightActive({ ...base, nightMode: "on" }, at(5, 12), 0)).toBe(true);
  });
});

describe("a manual change wins until the next boundary", () => {
  it("switched off at 23:00 stays off until 21:00 the next day", () => {
    let s = auto();
    expect(nightActive(s, at(5, 23), 0)).toBe(true);
    s = toggledNight(s, at(5, 23), 0);
    expect(s.nightMode).toBe("auto");
    expect(s.nightOverride).toEqual({ active: false, untilWallMs: at(6, 7) });
    expect(nightActive(s, at(6, 3), 0)).toBe(false);
    expect(nightActive(s, at(6, 7), 0)).toBe(false);
    expect(nightActive(s, at(6, 12), 0)).toBe(false);
    expect(nightActive(s, at(6, 21), 0)).toBe(true);
  });

  it("switched on in the day stays on until the window starts, then follows it", () => {
    const s = toggledNight(auto(), at(5, 12), 0);
    expect(s.nightOverride).toEqual({ active: true, untilWallMs: at(5, 21) });
    expect(nightActive(s, at(5, 20, 59), 0)).toBe(true);
    expect(nightActive(s, at(6, 3), 0)).toBe(true);
    expect(nightActive(s, at(6, 7), 0)).toBe(false);
  });

  it("ignores an override from another clock (more than a day ahead)", () => {
    const far = { active: true, untilWallMs: at(5, 12) + DAY + MIN };
    expect(nightActive(auto(far), at(5, 12), 0)).toBe(false);
  });

  it("flips Off and On directly, and an empty window makes the choice the mode", () => {
    const on = toggledNight(base, at(5, 12), 0);
    expect(on).toMatchObject({ nightMode: "on", nightOverride: null });
    expect(toggledNight(on, at(5, 12), 0).nightMode).toBe("off");
    const empty = { ...auto(), nightFromMinute: 60, nightToMinute: 60 };
    expect(toggledNight(empty, at(5, 12), 0)).toMatchObject({
      nightMode: "on",
      nightOverride: null,
    });
  });

  it("drops the override when the mode or the window changes", () => {
    const held = auto({ active: false, untilWallMs: at(6, 7) });
    expect(withNightMode(held, "auto").nightOverride).toBeNull();
    expect(withNightWindow(held, 22 * 60, 6 * 60)).toMatchObject({
      nightFromMinute: 22 * 60,
      nightOverride: null,
    });
  });
});

describe("storage", () => {
  const fake = (): Storage => {
    const map = new Map<string, string>();
    return {
      getItem: (k: string) => map.get(k) ?? null,
      setItem: (k: string, v: string) => void map.set(k, v),
    } as unknown as Storage;
  };

  it("round-trips the choice, the window and the override", () => {
    const storage = fake();
    const chosen = auto({ active: true, untilWallMs: 1_234_567 });
    storeNightSettings(storage, { ...chosen, nightFromMinute: 22 * 60 + 30, nightToMinute: 360 });
    expect(readNightSettings(storage)).toEqual({
      ...chosen,
      nightFromMinute: 22 * 60 + 30,
      nightToMinute: 360,
    });
  });

  it("reads nothing for garbage, and defaults a bad minute", () => {
    const storage = fake();
    storage.setItem(NIGHT_STORAGE_KEY, "not json");
    expect(readNightSettings(storage)).toBeNull();
    expect(parseNightSettings({ mode: "twilight" })).toBeNull();
    expect(parseNightSettings({ mode: "auto", from: 9999, to: -5 })).toMatchObject({
      nightFromMinute: FROM,
      nightToMinute: TO,
      nightOverride: null,
    });
    expect(normalizeMinute(1440, 7)).toBe(7);
    expect(
      readNightSettings({
        getItem: () => {
          throw new DOMException("blocked", "SecurityError");
        },
      }),
    ).toBeNull();
  });

  it("stores nothing for the defaults and writes when something changes", () => {
    const storage = fake();
    persistNightSettings(storage, base);
    expect(storage.getItem(NIGHT_STORAGE_KEY)).toBeNull();
    persistNightSettings(storage, { ...base, nightMode: "on" });
    expect(readNightSettings(storage)?.nightMode).toBe("on");
    persistNightSettings(storage, base);
    expect(readNightSettings(storage)?.nightMode).toBe("off");
  });

  it("converts time field values", () => {
    expect(minuteToTimeValue(7 * 60 + 5)).toBe("07:05");
    expect(timeValueToMinute("21:30")).toBe(21 * 60 + 30);
    expect(timeValueToMinute("")).toBeNull();
    expect(timeValueToMinute("24:00")).toBeNull();
  });
});
