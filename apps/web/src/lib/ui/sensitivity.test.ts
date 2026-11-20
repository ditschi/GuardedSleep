import { describe, expect, it } from "vitest";
import { dbfsOf, isSignalingUrl, meterFraction, sensitivityOf, thresholdOf } from "./sensitivity";

const dbfs = (threshold: number): number => 20 * Math.log10(threshold);

describe("sensitivity slider in decibels", () => {
  it("runs the other way round: more sensitive means a lower threshold", () => {
    expect(thresholdOf(60)).toBeLessThan(thresholdOf(40));
  });

  it("spans -55 dBFS (most sensitive) to -15 dBFS (least)", () => {
    expect(dbfs(thresholdOf(100))).toBeCloseTo(-55, 0);
    expect(dbfs(thresholdOf(0))).toBeCloseTo(-15, 0);
    expect(dbfsOf(100)).toBe(-55);
    expect(dbfsOf(0)).toBe(-15);
  });

  it("puts the middle at -35 dB, not at half of the linear range", () => {
    expect(dbfsOf(50)).toBe(-35);
    expect(dbfs(thresholdOf(50))).toBeCloseTo(-35, 1);
    // The linear midpoint of the two ends would be far louder.
    expect(thresholdOf(50)).toBeLessThan((thresholdOf(0) + thresholdOf(100)) / 2);
  });

  it("keeps the most sensitive threshold above the core's lowest limit", () => {
    // 0.0015 is the core's shared limit (`minNoiseThreshold()`), checked against the real core in
    // packages/core-wasm.
    expect(thresholdOf(100)).toBeGreaterThanOrEqual(0.0015);
  });

  it("clamps what it is given", () => {
    expect(thresholdOf(-20)).toBe(thresholdOf(0));
    expect(thresholdOf(400)).toBe(thresholdOf(100));
    expect(sensitivityOf(0.0001)).toBe(100);
    expect(sensitivityOf(0.9)).toBe(0);
  });

  it("round-trips the slider steps", () => {
    for (let step = 0; step <= 100; step += 5) {
      expect(Math.abs(sensitivityOf(thresholdOf(step)) - step)).toBeLessThanOrEqual(1);
    }
  });

  it("puts the default threshold (-36 dBFS) near the middle", () => {
    expect(sensitivityOf(0.016)).toBeGreaterThan(40);
    expect(sensitivityOf(0.016)).toBeLessThan(60);
  });
});

describe("meters on the slider's decibel scale", () => {
  it("runs from -60 dBFS (empty) to -10 dBFS (full)", () => {
    expect(meterFraction(0)).toBe(0);
    expect(meterFraction(10 ** (-60 / 20))).toBeCloseTo(0, 5);
    expect(meterFraction(10 ** (-35 / 20))).toBeCloseTo(0.5, 5);
    expect(meterFraction(10 ** (-10 / 20))).toBeCloseTo(1, 5);
    expect(meterFraction(1)).toBe(1);
    expect(meterFraction(Number.NaN)).toBe(0);
  });

  it("puts the threshold tick where the slider puts it: -dB per 100 steps is even", () => {
    // Equal slider steps move the tick by equal amounts, the more sensitive the further left.
    const tick = (p: number): number => meterFraction(thresholdOf(p));
    expect(tick(100)).toBeLessThan(tick(50));
    expect(tick(50) - tick(100)).toBeCloseTo(tick(0) - tick(50), 1);
    expect(tick(100)).toBeCloseTo(0.1, 1);
  });
});

describe("isSignalingUrl", () => {
  it.each(["ws://192.168.1.5:8787", "wss://relay.example.test/path", " wss://x.test "])(
    "accepts %s",
    (url) => expect(isSignalingUrl(url)).toBe(true),
  );

  it.each(["", "http://relay.example.test", "relay.example.test", "wss://", "javascript:alert(1)"])(
    "rejects %j",
    (url) => expect(isSignalingUrl(url)).toBe(false),
  );
});
