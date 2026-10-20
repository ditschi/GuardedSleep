import { describe, expect, it } from "vitest";
import { shouldCompleteSlideUp, slideUpProgress } from "./slide-up-gesture.js";

describe("slide-up-gesture", () => {
  it("completes when sliding up past threshold", () => {
    expect(shouldCompleteSlideUp(-80, 72)).toBe(true);
    expect(shouldCompleteSlideUp(-40, 72)).toBe(false);
  });

  it("computes progress toward threshold", () => {
    expect(slideUpProgress(-36, 72)).toBe(0.5);
    expect(slideUpProgress(10, 72)).toBe(0);
  });
});
