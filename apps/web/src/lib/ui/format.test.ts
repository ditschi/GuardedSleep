import { describe, expect, it } from "vitest";
import { createTranslator } from "../i18n/translate";
import {
  formatClock,
  formatCountdown,
  formatDuration,
  formatDurationWords,
  levelWordKey,
  remainingMs,
  spokenDuration,
} from "./format";

describe("formatCountdown", () => {
  it.each([
    [300_000, "5:00"],
    [272_000, "4:32"],
    [65_000, "1:05"],
    [59_001, "1:00"],
    [1, "0:01"],
    [0, "0:00"],
    [-5_000, "0:00"],
  ])("%d ms shows %s", (ms, text) => {
    expect(formatCountdown(ms)).toBe(text);
  });
});

describe("remainingMs", () => {
  it("counts down to the deadline and stops at zero", () => {
    expect(remainingMs(10_000, 4_000)).toBe(6_000);
    expect(remainingMs(10_000, 12_000)).toBe(0);
  });

  it("has nothing to count without a deadline", () => {
    expect(remainingMs(null, 4_000)).toBeNull();
  });
});

describe("formatDuration", () => {
  const t = createTranslator("en");
  it.each([
    [0, "1 s"],
    [45_000, "45 s"],
    [120_000, "2 min"],
    [3_600_000, "1 h"],
    [3_900_000, "1 h 5 min"],
  ])("%d ms reads %s", (ms, text) => {
    expect(formatDuration(t, ms)).toBe(text);
  });
});

describe("formatClock", () => {
  it("shows hours and minutes", () => {
    expect(formatClock(Date.UTC(2026, 9, 23, 22, 41), "en")).toMatch(/\d{1,2}:\d{2}/);
  });
});

describe("levelWordKey", () => {
  it("names quiet, some sound and loud", () => {
    expect(levelWordKey(0)).toBe("native.ui.parent.level.quiet");
    expect(levelWordKey(0.2)).toBe("native.ui.parent.level.some");
    expect(levelWordKey(0.8)).toBe("native.ui.parent.level.loud");
  });
});

describe("durations in words", () => {
  const t = createTranslator("en");
  it.each([
    [48_000, "48 s"],
    [108_000, "1 min 48 s"],
    [120_000, "2 min"],
  ])("%d ms reads %s in running text", (ms, text) => {
    expect(formatDurationWords(t, ms)).toBe(text);
  });

  it.each([
    [1_000, "1 second"],
    [12_000, "12 seconds"],
    [60_000, "1 minute"],
    [72_000, "1 minute 12 seconds"],
    [135_000, "2 minutes 15 seconds"],
  ])("the counter at %d ms is spoken as '%s'", (ms, text) => {
    expect(spokenDuration(t, ms)).toBe(text);
  });

  it("speaks German with the right plural", () => {
    const de = createTranslator("de");
    expect(spokenDuration(de, 62_000)).toBe("1 Minute 2 Sekunden");
  });
});
