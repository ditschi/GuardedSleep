import { describe, expect, it } from "vitest";
import {
  SESSION_AUTO_RECONNECT_MAX_ATTEMPTS,
  sessionReconnectDelayMs,
} from "./session-auto-reconnect.js";

describe("session-auto-reconnect", () => {
  it("respects bounds", () => {
    expect(sessionReconnectDelayMs(1)).toBeGreaterThanOrEqual(1400);
    expect(sessionReconnectDelayMs(1)).toBeLessThanOrEqual(45_000);
    expect(sessionReconnectDelayMs(80)).toBe(45_000);
    expect(SESSION_AUTO_RECONNECT_MAX_ATTEMPTS).toBeGreaterThan(0);
  });
});
