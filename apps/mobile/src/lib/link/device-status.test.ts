import { describe, expect, it } from "vitest";
import {
  bucketBattery,
  computeUnitWarnings,
  mergeBabyDeviceStatus,
  RESUMED_WARNING_MS,
  type UnitSignals,
} from "./device-status.js";

const calm: UnitSignals = {
  pageHidden: false,
  wakeLockHeld: true,
  lastGapWallMs: null,
  nowWallMs: 1_000_000,
  charging: true,
  micEnded: false,
};

describe("computeUnitWarnings", () => {
  it("reports nothing for a calm, plugged-in, visible unit", () => {
    expect(computeUnitWarnings(calm)).toEqual([]);
  });

  it("reports each condition", () => {
    expect(
      computeUnitWarnings({
        ...calm,
        pageHidden: true,
        wakeLockHeld: false,
        charging: false,
        micEnded: true,
      }),
    ).toEqual(["page_hidden", "wake_lock_off", "not_charging", "mic_permission_lost"]);
  });

  it("does not claim a charging state the browser does not tell", () => {
    expect(computeUnitWarnings({ ...calm, charging: null })).toEqual([]);
  });

  it("keeps resumed_from_sleep for 10 minutes after a gap", () => {
    const gap = 1_000_000 - (RESUMED_WARNING_MS - 1);
    expect(computeUnitWarnings({ ...calm, lastGapWallMs: gap })).toEqual(["resumed_from_sleep"]);
    expect(computeUnitWarnings({ ...calm, lastGapWallMs: 1_000_000 - RESUMED_WARNING_MS })).toEqual(
      [],
    );
  });
});

describe("mergeBabyDeviceStatus", () => {
  it("adds warnings only when there are some", () => {
    expect(mergeBabyDeviceStatus({ battery_pct: 80, charging: true }, [])).toEqual({
      battery_pct: 80,
      charging: true,
    });
    expect(mergeBabyDeviceStatus({ battery_pct: 80 }, ["page_hidden"])).toEqual({
      battery_pct: 80,
      unit_warnings: ["page_hidden"],
    });
  });
});

describe("bucketBattery", () => {
  it("rounds to steps of 5", () => {
    expect(bucketBattery(0.77)).toBe(75);
  });
});
