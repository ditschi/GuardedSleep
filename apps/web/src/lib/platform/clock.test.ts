import { describe, expect, it } from "vitest";
import { createClock, SLEEP_GAP_MS, SleepDetector } from "./clock";

describe("SleepDetector", () => {
  it("sees nothing while both clocks move together", () => {
    const d = new SleepDetector();
    expect(d.observe(0, 1_000_000)).toBe(0);
    expect(d.observe(250, 1_000_250)).toBe(0);
    expect(d.observe(60_250, 1_060_250)).toBe(0);
  });

  it("reports the surplus of the wall clock once it reaches the threshold", () => {
    const d = new SleepDetector();
    d.observe(1_000, 5_000_000);
    // Slept for an hour: the monotonic clock moved 300 ms, the wall clock 3 600 300 ms.
    expect(d.observe(1_300, 5_000_000 + 3_600_300)).toBe(3_600_000);
  });

  it("ignores a surplus below the threshold and a wall clock that moves backwards", () => {
    const d = new SleepDetector();
    d.observe(0, 10_000);
    expect(d.observe(1_000, 10_000 + 1_000 + SLEEP_GAP_MS - 1)).toBe(0);
    expect(d.observe(2_000, 1_000)).toBe(0);
  });
});

describe("createClock", () => {
  function fakes() {
    const t = { perf: 0, wall: 1_700_000_000_000 };
    const clock = createClock({ now: () => t.perf }, () => t.wall);
    return { t, clock };
  }

  it("adds the time a sleeping device did not count, once, and tells the listeners", () => {
    const { t, clock } = fakes();
    const gaps: Array<{ gapMs: number; wallMs: number }> = [];
    clock.onGap((g) => gaps.push(g));
    expect(clock.elapsedMs()).toBe(0);
    t.perf += 250;
    t.wall += 250;
    expect(clock.elapsedMs()).toBe(250);
    // The lid closes for 8 minutes: only the wall clock moves.
    t.wall += 8 * 60_000;
    t.perf += 100;
    t.wall += 100;
    expect(clock.elapsedMs()).toBe(250 + 100 + 8 * 60_000);
    expect(gaps).toEqual([{ gapMs: 8 * 60_000, wallMs: t.wall }]);
    // Afterwards it ticks normally.
    t.perf += 250;
    t.wall += 250;
    expect(clock.elapsedMs()).toBe(250 + 100 + 8 * 60_000 + 250);
    expect(gaps).toHaveLength(1);
  });

  it("never goes backwards and can be checked without reading the time", () => {
    const { t, clock } = fakes();
    const gaps: number[] = [];
    clock.onGap((g) => gaps.push(g.gapMs));
    clock.check();
    t.wall += 30_000;
    clock.check();
    expect(gaps).toEqual([30_000]);
    expect(clock.elapsedMs()).toBe(30_000);
    t.perf -= 5_000;
    expect(clock.elapsedMs()).toBe(30_000);
  });

  it("stops notifying after the unsubscribe", () => {
    const { t, clock } = fakes();
    let calls = 0;
    const off = clock.onGap(() => calls++);
    clock.check();
    off();
    t.wall += 60_000;
    clock.check();
    expect(calls).toBe(0);
  });
});
