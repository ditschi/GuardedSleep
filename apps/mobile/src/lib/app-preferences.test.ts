import { afterEach, describe, expect, it, vi } from "vitest";
import {
  applyAppTheme,
  heartbeatIntervalMs,
  persistAppPreferences,
  readAppPreferences,
  resolveThemeDark,
  THEME_CHANGE_EVENT,
} from "./app-preferences.js";

describe("app-preferences", () => {
  afterEach(() => {
    localStorage.clear();
    document.documentElement.removeAttribute("data-theme");
    document.documentElement.style.colorScheme = "";
  });

  it("returns defaults when storage is empty", () => {
    expect(readAppPreferences().onboardingCompleted).toBe(false);
    expect(readAppPreferences().wakeLockInListenMode).toBe(true);
    expect(readAppPreferences().nightMode).toBe("system");
  });

  it("round-trips patches", () => {
    persistAppPreferences({ onboardingCompleted: true, nightMode: "light" });
    const prefs = readAppPreferences();
    expect(prefs.onboardingCompleted).toBe(true);
    expect(prefs.nightMode).toBe("light");
  });

  it("clamps invalid heartbeat interval", () => {
    persistAppPreferences({ heartbeatIntervalSec: 99 });
    expect(readAppPreferences().heartbeatIntervalSec).toBe(10);
    expect(heartbeatIntervalMs()).toBe(10_000);
  });

  it("falls back to system for invalid nightMode", () => {
    localStorage.setItem("guardedsleep:app-preferences", JSON.stringify({ nightMode: "sepia" }));
    expect(readAppPreferences().nightMode).toBe("system");
  });

  it("resolveThemeDark respects explicit light and dark", () => {
    expect(resolveThemeDark({ ...readAppPreferences(), nightMode: "light" })).toBe(false);
    expect(resolveThemeDark({ ...readAppPreferences(), nightMode: "dark" })).toBe(true);
  });

  it("applyAppTheme sets document dataset and dispatches change event", () => {
    const handler = vi.fn();
    window.addEventListener(THEME_CHANGE_EVENT, handler);
    persistAppPreferences({ nightMode: "light" });
    applyAppTheme();
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(handler).toHaveBeenCalledTimes(1);
    window.removeEventListener(THEME_CHANGE_EVENT, handler);
  });
});
