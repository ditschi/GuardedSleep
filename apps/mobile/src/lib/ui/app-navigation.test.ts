import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("$app/navigation", () => ({
  goto: vi.fn(),
}));

import { goto } from "$app/navigation";
import {
  canNavigateBack,
  navigateAppBack,
  pushBackInterceptor,
  resolveBackFallback,
  shouldShowAppBack,
} from "./app-navigation.js";

/** The test replaces `history` with a plain stub whose `state` is writable. */
function setHistoryState(state: unknown): void {
  (history as { state: unknown }).state = state;
}

describe("app-navigation", () => {
  const back = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("history", {
      length: 1,
      state: { "sveltekit:index": 0 },
      back,
    });
    vi.stubGlobal("window", {
      location: { pathname: "/baby", search: "" },
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("hides back on home and onboarding", () => {
    expect(shouldShowAppBack("/")).toBe(false);
    expect(shouldShowAppBack("/onboard")).toBe(false);
    expect(shouldShowAppBack("/baby")).toBe(true);
  });

  it("resolves session fallbacks to home", () => {
    expect(resolveBackFallback("/baby", new URLSearchParams())).toBe("/");
    expect(resolveBackFallback("/parent", new URLSearchParams())).toBe("/");
    expect(resolveBackFallback("/pair", new URLSearchParams())).toBe("/");
  });

  it("resolves settings return links", () => {
    const q = new URLSearchParams({
      returnTo: "baby",
      room: "test-room",
      signal: "wss://lan/signaling",
    });
    expect(resolveBackFallback("/settings", q)).toBe(
      "/baby?room=test-room&signal=wss%3A%2F%2Flan%2Fsignaling",
    );
  });

  it("uses sveltekit history index when available", () => {
    setHistoryState({ "sveltekit:index": 2 });
    expect(canNavigateBack()).toBe(true);
    setHistoryState({ "sveltekit:index": 0 });
    expect(canNavigateBack()).toBe(false);
  });

  it("runs interceptors before history.back", async () => {
    setHistoryState({ "sveltekit:index": 2 });
    const remove = pushBackInterceptor(() => true);
    await navigateAppBack("/");
    expect(back).not.toHaveBeenCalled();
    expect(goto).not.toHaveBeenCalled();
    remove();
  });

  it("falls back to goto when history cannot go back", async () => {
    await navigateAppBack("/");
    expect(back).not.toHaveBeenCalled();
    expect(goto).toHaveBeenCalledWith("/");
  });
});
