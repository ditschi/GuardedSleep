import { afterEach, describe, expect, it } from "vitest";
import { HOLD_DURATION_DEFAULT_MS } from "./hold-duration-presets.js";
import { persistBabyNoiseGate, readBabyUnitPreferences } from "./baby-unit-preferences.js";

describe("baby-unit-preferences", () => {
  afterEach(() => {
    localStorage.clear();
  });

  it("round-trips noise gate", () => {
    persistBabyNoiseGate(0.06, 12_000);
    const p = readBabyUnitPreferences();
    expect(p.threshold).toBe(0.06);
    expect(p.holdMs).toBe(12_000);
    expect(p.kioskImmersive).toBe(true);
  });

  it("uses defaults when storage empty", () => {
    const p = readBabyUnitPreferences();
    expect(p.threshold).toBe(0.035);
    expect(p.holdMs).toBe(HOLD_DURATION_DEFAULT_MS);
  });
});
