import { afterEach, describe, expect, it, vi } from "vitest";
import {
  androidDevWssSignalingEnabled,
  shouldUseAndroidNativeWss,
} from "./android-dev-wss-enabled.js";

vi.mock("@capacitor/core", () => ({
  Capacitor: { getPlatform: () => "android" },
}));

describe("android-dev-wss-enabled", () => {
  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("enables native wss on android when dev hints are on", () => {
    vi.stubEnv("VITE_DEV_CONNECT_HINTS", "true");
    expect(androidDevWssSignalingEnabled()).toBe(true);
    expect(shouldUseAndroidNativeWss("wss://192.168.1.10:8443/signaling")).toBe(true);
    expect(shouldUseAndroidNativeWss("ws://192.168.1.10:8787")).toBe(false);
  });
});
