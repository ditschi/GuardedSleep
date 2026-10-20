import { describe, expect, it } from "vitest";
import { alarmModeFor, LEVEL2_BEEP_INTERVAL_MS } from "./alarm-tone.js";

describe("connection alarm mode", () => {
  it("beeps every 10 s at level 2, plays continuously at level 3, stays silent otherwise", () => {
    expect(LEVEL2_BEEP_INTERVAL_MS).toBe(10_000);
    expect(alarmModeFor(0, false)).toBe("off");
    expect(alarmModeFor(1, false)).toBe("off");
    expect(alarmModeFor(2, false)).toBe("beep");
    expect(alarmModeFor(3, false)).toBe("continuous");
  });

  it("is muted by 'I'm checking'", () => {
    expect(alarmModeFor(2, true)).toBe("off");
    expect(alarmModeFor(3, true)).toBe("off");
  });
});
