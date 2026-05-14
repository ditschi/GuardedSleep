import { afterEach, describe, expect, it } from "vitest";
import { persistLastSnoozeMinutes, readLastSnoozeMinutes } from "./parent-snooze-preferences.js";

describe("parent-snooze-preferences", () => {
  afterEach(() => {
    localStorage.clear();
  });

  it("round-trips last snooze minutes", () => {
    persistLastSnoozeMinutes(25);
    expect(readLastSnoozeMinutes()).toBe(25);
  });

  it("clamps invalid stored values", () => {
    localStorage.setItem("guardedsleep:parent:snooze-last-minutes", "999");
    expect(readLastSnoozeMinutes()).toBe(10);
  });
});
