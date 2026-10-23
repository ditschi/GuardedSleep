import { describe, expect, it } from "vitest";
import { createClock } from "./clock";

describe("createClock", () => {
  function fake(start = 1_700_000_000_000.7) {
    const t = { wall: start };
    return { t, clock: createClock(() => t.wall) };
  }

  it("reads whole milliseconds of the wall clock", () => {
    const { clock } = fake();
    expect(clock.elapsedMs()).toBe(1_700_000_000_000);
    expect(clock.wallMs()).toBe(1_700_000_000_000.7);
  });

  it("gives the same value when the wall clock jumps backwards, then follows it again", () => {
    const { t, clock } = fake(1_700_000_005_000);
    expect(clock.elapsedMs()).toBe(1_700_000_005_000);
    t.wall -= 3_000;
    expect(clock.elapsedMs()).toBe(1_700_000_005_000);
    t.wall += 2_000;
    expect(clock.elapsedMs()).toBe(1_700_000_005_000);
    t.wall += 1_500;
    expect(clock.elapsedMs()).toBe(1_700_000_005_500);
  });

  it("passes a large forward jump (the device slept) through as elapsed time", () => {
    const { t, clock } = fake(1_700_000_000_000);
    const before = clock.elapsedMs();
    t.wall += 45 * 60_000;
    expect(clock.elapsedMs() - before).toBe(45 * 60_000);
  });
});
