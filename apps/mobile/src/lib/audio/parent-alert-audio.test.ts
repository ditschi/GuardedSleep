import { afterEach, describe, expect, it } from "vitest";
import { persistAppPreferences } from "$lib/app-preferences.js";
import { resolveAlertPlaybackGain } from "./parent-alert-audio.js";

describe("parent-alert-audio", () => {
  afterEach(() => {
    localStorage.clear();
  });

  it("clamps gain to configured range", () => {
    persistAppPreferences({ alertPlaybackGain: 1.5 });
    expect(resolveAlertPlaybackGain(true)).toBe(1.5);
  });

  it("applies minimum gain floor during alert when enabled", () => {
    persistAppPreferences({ alertPlaybackGain: 0.5, alertMinGainDuringAlert: true });
    expect(resolveAlertPlaybackGain(true)).toBe(0.8);
    expect(resolveAlertPlaybackGain(false)).toBe(0.5);
  });
});
