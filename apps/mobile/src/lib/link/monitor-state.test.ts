import { describe, expect, it } from "vitest";
import type { LinkEvent, LinkStatus } from "@guardedsleep/core-wasm";
import {
  deriveSilence,
  mapLinkStatus,
  meterLevel,
  meterWord,
  NOISE_EVENT_GAP_MS,
  reduceParentEvent,
  tickParentState,
} from "./monitor-state.js";
import { formatClockDuration, formatDuration } from "./format.js";
import { initialLinkUi, initialParentState } from "./ui-contract.js";

const ctx = (wall: number, elapsed = wall) => ({ nowWallMs: wall, nowElapsedMs: elapsed });
const status = (over: Partial<LinkStatus>): LinkStatus => ({
  state: "connected",
  level: 0,
  key: null,
  cause: null,
  acknowledged_until: null,
  last_alive_ms: null,
  ...over,
});
const ev = (e: LinkEvent): LinkEvent => e;

describe("link status mapping", () => {
  it("maps escalation level and cause only while not connected", () => {
    const l = mapLinkStatus(
      initialLinkUi(),
      status({ state: "reconnecting", level: 2, cause: "peer_absent" }),
      ctx(10),
    );
    expect(l).toMatchObject({ phase: "reconnecting", level: 2, causeHint: "peer_absent" });
    const ok = mapLinkStatus(l, status({ state: "connected", level: 2 }), ctx(20));
    expect(ok).toMatchObject({ phase: "connected", level: 0, lastAliveElapsedMs: 20 });
  });

  it("takes the last contact time from the core while not connected", () => {
    const l = mapLinkStatus(
      initialLinkUi(),
      status({ state: "reconnecting", level: 1, last_alive_ms: 4_000 }),
      ctx(90_000),
    );
    expect(l.lastAliveElapsedMs).toBe(4_000);
    const never = mapLinkStatus(l, status({ state: "connecting" }), ctx(95_000));
    expect(never.lastAliveElapsedMs).toBeNull();
  });

  it("keeps the terminal failure key and ignores unknown cause strings", () => {
    const l = mapLinkStatus(
      initialLinkUi(),
      status({ state: "failed_needs_user", key: "pairing_key_mismatch", cause: "bogus" }),
      ctx(1),
    );
    expect(l.failureKey).toBe("pairing_key_mismatch");
    expect(l.causeHint).toBeNull();
  });
});

describe("parent reducer", () => {
  it("never shows 'all quiet' while disconnected and restores the old start when nothing was missed", () => {
    let s = reduceParentEvent(
      initialParentState(),
      ev({ type: "status", status: status({}) }),
      ctx(1000),
    );
    expect(s.quietSinceWallMs).toBe(1000);
    s = reduceParentEvent(
      s,
      ev({ type: "status", status: status({ state: "reconnecting", level: 1 }) }),
      ctx(50_000),
    );
    expect(s.quietSinceWallMs).toBeNull();
    s = reduceParentEvent(s, ev({ type: "status", status: status({}) }), ctx(60_000));
    expect(s.quietSinceWallMs).toBe(60_000);
    s = reduceParentEvent(
      s,
      ev({ type: "missed_events", count: 0, max_rms: 0, complete: true, down_ms: 10_000 }),
      ctx(60_100),
    );
    expect(s.quietSinceWallMs).toBe(1000);
    expect(s.missed).toMatchObject({ count: 0, complete: true });
  });

  it("hides the missed count when the baby's buffer was incomplete", () => {
    const s = reduceParentEvent(
      initialParentState(),
      ev({ type: "missed_events", count: 3, max_rms: 0.4, complete: false, down_ms: 9000 }),
      ctx(1),
    );
    expect(s.missed).toMatchObject({ count: null, complete: false, maxRms: 0.4 });
  });

  it("groups noise bursts less than 2 minutes apart into one event", () => {
    let s = reduceParentEvent(
      initialParentState(),
      ev({ type: "noise_alert", rms: 0.3, probe: false, snoozed: false }),
      ctx(1_000_000),
    );
    expect(s.noiseAlertActive).toBe(true);
    expect(s.alert).toMatchObject({ sinceWallMs: 1_000_000, probe: false });
    s = { ...s, alert: { ...s.alert!, soundStopped: true } };
    s = reduceParentEvent(
      s,
      ev({ type: "noise_alert", rms: 0.3, probe: false, snoozed: false }),
      ctx(1_060_000),
    );
    expect(s.alert).toMatchObject({
      sinceWallMs: 1_000_000,
      lastWallMs: 1_060_000,
      soundStopped: true,
    });
    s = reduceParentEvent(
      s,
      ev({ type: "noise_alert", rms: 0.3, probe: false, snoozed: false }),
      ctx(1_060_000 + NOISE_EVENT_GAP_MS + 1),
    );
    expect(s.alert?.soundStopped).toBe(false);
    expect(s.alert?.sinceWallMs).toBe(1_060_000 + NOISE_EVENT_GAP_MS + 1);
  });

  it("ends the event after 2 quiet minutes and resumes the quiet period at the last noise", () => {
    let s = reduceParentEvent(
      initialParentState(),
      ev({ type: "status", status: status({}) }),
      ctx(1),
    );
    s = reduceParentEvent(
      s,
      ev({ type: "noise_alert", rms: 0.3, probe: false, snoozed: false }),
      ctx(5000),
    );
    expect(tickParentState(s, ctx(5000 + NOISE_EVENT_GAP_MS - 1)).noiseAlertActive).toBe(true);
    const ended = tickParentState(s, ctx(5000 + NOISE_EVENT_GAP_MS));
    expect(ended).toMatchObject({ noiseAlertActive: false, alert: null, quietSinceWallMs: 5000 });
  });

  it("records snoozed noise without opening an alert, and marks test alerts as probes", () => {
    let s = reduceParentEvent(
      initialParentState(),
      ev({ type: "noise_alert", rms: 0.3, probe: false, snoozed: true }),
      ctx(10),
    );
    expect(s).toMatchObject({ alert: null, noiseAlertActive: false, lastNoiseSnoozed: true });
    s = reduceParentEvent(
      s,
      ev({ type: "noise_alert", rms: 0.5, probe: true, snoozed: true }),
      ctx(20),
    );
    expect(s.alert?.probe).toBe(true);
    expect(s.lastNoiseWallMs).toBe(10);
  });

  it("expires a snooze on the tick", () => {
    const s = { ...initialParentState(), snoozedUntilWallMs: 100 };
    expect(tickParentState(s, ctx(99)).snoozedUntilWallMs).toBe(100);
    expect(tickParentState(s, ctx(100)).snoozedUntilWallMs).toBeNull();
  });

  it("flags a web baby and reports silence as unknown for it", () => {
    const s = reduceParentEvent(
      initialParentState(),
      ev({ type: "peer_status", status: { battery_pct: 55, charging: false } }),
      ctx(1),
    );
    expect(s.peer).toMatchObject({ isWebClient: true, silence: "unknown", batteryPct: 55 });
  });

  it("clears the low battery chip once the baby charges", () => {
    let s = reduceParentEvent(
      initialParentState(),
      ev({ type: "low_battery", pct: 10, critical: false }),
      ctx(1),
    );
    expect(s.peer.lowBattery).toEqual({ pct: 10, critical: false });
    s = reduceParentEvent(
      s,
      ev({ type: "peer_status", status: { battery_pct: 15, charging: true } }),
      ctx(2),
    );
    expect(s.peer.lowBattery).toBeNull();
  });
});

describe("silence derivation", () => {
  it.each([
    [{ dnd: "total" }, "silent"],
    [{ ringer: "silent" }, "silent"],
    [{ quiet_active: true, ringer: "normal" }, "silent"],
    [{ ringer: "vibrate", dnd: "off", dnd_access: true }, "may_buzz"],
    [{ ringer: "normal", dnd: "off", dnd_access: true }, "may_ring"],
    [{ ringer: "normal", dnd: "off", dnd_access: false }, "no_access"],
    [{ battery_pct: 40 }, "unknown"],
  ] as const)("%j -> %s", (input, expected) => {
    expect(deriveSilence(input)).toBe(expected);
  });
});

describe("meter and formatting", () => {
  it("scales and words the level", () => {
    expect(meterLevel(null)).toBe(0);
    expect(meterLevel(0.1)).toBeCloseTo(0.5);
    expect(meterLevel(5)).toBe(1);
    expect(meterWord(0.05)).toBe("quiet");
    expect(meterWord(0.3)).toBe("some");
    expect(meterWord(0.9)).toBe("loud");
  });

  it("formats durations as the catalogue expects", () => {
    expect(formatDuration(45_000)).toBe("45 s");
    expect(formatDuration(120_000)).toBe("2 min");
    expect(formatDuration(3_900_000)).toBe("1 h 5 min");
    expect(formatClockDuration(125_000)).toBe("2:05");
  });
});
