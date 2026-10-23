import { describe, expect, it } from "vitest";
import {
  isSignalingUrl,
  sensitivityOf,
  THRESHOLD_MAX,
  THRESHOLD_MIN,
  thresholdOf,
} from "./sensitivity";

describe("sensitivity slider", () => {
  it("runs the other way round: more sensitive means a lower threshold", () => {
    expect(thresholdOf(0)).toBe(THRESHOLD_MAX);
    expect(thresholdOf(100)).toBe(THRESHOLD_MIN);
    expect(thresholdOf(60)).toBeLessThan(thresholdOf(40));
  });

  it("clamps what it is given", () => {
    expect(thresholdOf(-20)).toBe(THRESHOLD_MAX);
    expect(thresholdOf(400)).toBe(THRESHOLD_MIN);
    expect(sensitivityOf(0)).toBe(100);
    expect(sensitivityOf(1)).toBe(0);
  });

  it("round-trips the slider steps", () => {
    for (let step = 0; step <= 100; step += 5) {
      expect(Math.abs(sensitivityOf(thresholdOf(step)) - step)).toBeLessThanOrEqual(2);
    }
  });

  it("puts the default threshold inside the range", () => {
    const s = sensitivityOf(0.12);
    expect(s).toBeGreaterThan(0);
    expect(s).toBeLessThan(100);
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
