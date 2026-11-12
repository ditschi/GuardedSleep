import { describe, expect, it, vi } from "vitest";
import type { AlertSink } from "./alert-sink";
import { BabyTalkModel, preemptingAlerts } from "./talk-model";
import { FakeTime } from "./test-support";

const sink = (): AlertSink => ({
  linkState: vi.fn(),
  linkLost: vi.fn(),
  noiseAlert: vi.fn(),
  noiseEnded: vi.fn(),
  stopNoiseSound: vi.fn(),
  notice: vi.fn(),
  clear: vi.fn(),
});

describe("preemptingAlerts", () => {
  it("ends a talk before a sound alert and passes everything on", () => {
    const inner = sink();
    const onSound = vi.fn();
    const alerts = preemptingAlerts(inner, onSound);
    alerts.noiseAlert("new", false, false);
    expect(onSound).toHaveBeenCalledTimes(1);
    expect(inner.noiseAlert).toHaveBeenCalledWith("new", false, false);
    alerts.notice({ type: "peer_stopped" });
    expect(onSound).toHaveBeenCalledTimes(2);
    alerts.linkState({
      phase: "reconnecting",
      level: 3,
      acknowledged: false,
      causeHint: null,
      wasConnected: true,
    });
    expect(onSound).toHaveBeenCalledTimes(3);
  });

  it("stays quiet for events that make no sound", () => {
    const onSound = vi.fn();
    const alerts = preemptingAlerts(sink(), onSound);
    alerts.noiseAlert("new", false, true);
    alerts.notice({ type: "low_battery", pct: 10, critical: false });
    alerts.linkState({
      phase: "connected",
      level: 0,
      acknowledged: false,
      causeHint: null,
      wasConnected: true,
    });
    alerts.linkState({
      phase: "reconnecting",
      level: 1,
      acknowledged: false,
      causeHint: null,
      wasConnected: true,
    });
    expect(onSound).not.toHaveBeenCalled();
  });
});

describe("BabyTalkModel", () => {
  it("keeps the last talk with the parent's name across runs", () => {
    const time = new FakeTime();
    const model = new BabyTalkModel(
      time,
      () => "Chrome on Android",
      () => undefined,
    );
    model.onEvent({ type: "talk_active", active: true, duration_ms: 0 });
    expect(model.state.parentTalking).toBe(true);
    time.advance(5_000);
    model.onEvent({ type: "talk_active", active: false, duration_ms: 5_000 });
    expect(model.state.lastTalk).toMatchObject({ name: "Chrome on Android", durationMs: 5_000 });
    model.reset();
    expect(model.state.parentTalking).toBe(false);
    expect(model.state.lastTalk).not.toBeNull();
  });
});
