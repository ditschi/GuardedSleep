import { describe, expect, it, vi } from "vitest";
import type { AlertSink } from "./alert-sink";
import { BabyTalkModel, ParentTalkModel, preemptingAlerts } from "./talk-model";
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

describe("ParentTalkModel: own microphone hint", () => {
  function talking() {
    const time = new FakeTime();
    const model = new ParentTalkModel(time, () => undefined);
    model.setConnected(true);
    model.onEvent(
      { type: "talk_changed", state: { state: "talking", until_ms: time.now + 30_000 } },
      time.now,
    );
    return { time, model };
  }

  it("shows after 2 s without sound while talking, and clears with sound or the end", () => {
    const { time, model } = talking();
    model.onMicLevel(0, time.now);
    time.advance(1_900);
    model.onMicLevel(0.001, time.now);
    expect(model.state.talkMicSilent).toBe(false);
    time.advance(200);
    model.onMicLevel(0, time.now);
    expect(model.state.talkMicSilent).toBe(true);
    model.onMicLevel(0.3, time.now);
    expect(model.state.talkMicSilent).toBe(false);
    time.advance(2_000);
    model.onMicLevel(0, time.now);
    time.advance(2_000);
    model.onMicLevel(0, time.now);
    expect(model.state.talkMicSilent).toBe(true);
    model.onEvent(
      { type: "talk_changed", state: { state: "ended", reason: "released" } },
      time.now,
    );
    expect(model.state.talkMicSilent).toBe(false);
  });

  it("ignores levels outside a talk", () => {
    const time = new FakeTime();
    const model = new ParentTalkModel(time, () => undefined);
    model.onMicLevel(0, time.now);
    time.advance(5_000);
    model.onMicLevel(0, time.now);
    expect(model.state.talkMicSilent).toBe(false);
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
    expect(model.state.lastTalk).toMatchObject({
      name: "Chrome on Android",
      durationMs: 5_000,
      // The time is the talk's start, not its end.
      atWallMs: time.wallMs() - 5_000,
    });
    model.reset();
    expect(model.state.parentTalking).toBe(false);
    expect(model.state.lastTalk).not.toBeNull();
  });
});
