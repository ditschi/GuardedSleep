import { describe, expect, it } from "vitest";
import {
  formatHoldDurationLabel,
  HOLD_DURATION_DEFAULT_MS,
  HOLD_DURATION_PRESET_MS,
} from "./hold-duration-presets.js";

describe("hold-duration-presets", () => {
  it("matches spec presets and default", () => {
    expect([...HOLD_DURATION_PRESET_MS]).toEqual([3000, 5000, 10_000, 15_000]);
    expect(HOLD_DURATION_DEFAULT_MS).toBe(5000);
  });

  it("formats labels without jargon overload", () => {
    expect(formatHoldDurationLabel(500)).toBe("500 ms");
    expect(formatHoldDurationLabel(3000)).toBe("3 s");
    expect(formatHoldDurationLabel(5500)).toBe("5.5 s");
  });
});
