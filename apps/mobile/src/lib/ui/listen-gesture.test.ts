import { describe, expect, it } from "vitest";
import { nextPhaseAfterRelease, shouldLockOnSlide } from "./listen-gesture.js";

describe("listen-gesture", () => {
  it("locks when slide exceeds threshold upward", () => {
    expect(shouldLockOnSlide(-40, 48)).toBe(false);
    expect(shouldLockOnSlide(-48, 48)).toBe(true);
    expect(shouldLockOnSlide(-80, 48)).toBe(true);
  });

  it("returns idle after release when not locked", () => {
    expect(nextPhaseAfterRelease("holding", false)).toBe("idle");
    expect(nextPhaseAfterRelease("holding", true)).toBe("locked");
    expect(nextPhaseAfterRelease("locked", false)).toBe("locked");
  });
});
