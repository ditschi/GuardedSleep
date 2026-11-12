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

describe("ParentTalkModel: result notices", () => {
  function model() {
    const time = new FakeTime();
    const m = new ParentTalkModel(time, () => undefined);
    m.setConnected(true);
    return { time, m };
  }
  const ended = (m: ParentTalkModel, now: number, reason: "no_answer" | "released" | "stopped") =>
    m.onEvent({ type: "talk_changed", state: { state: "ended", reason } }, now);

  it("shows an end reason for 8 s", () => {
    const { time, m } = model();
    ended(m, time.now, "no_answer");
    time.advance(7_999);
    m.onTick(time.now);
    expect(m.state.talkEnd).toBe("no_answer");
    time.advance(1);
    m.onTick(time.now);
    expect(m.state.talkEnd).toBeNull();
  });

  it("shows a denial for 8 s", () => {
    const { time, m } = model();
    m.onEvent(
      { type: "talk_changed", state: { state: "denied", reason: { kind: "off" } } },
      time.now,
    );
    time.advance(7_999);
    m.onTick(time.now);
    expect(m.state.talk.kind).toBe("denied");
    time.advance(1);
    m.onTick(time.now);
    expect(m.state.talk.kind).not.toBe("denied");
  });

  it("starts no timer for a release or a stop", () => {
    const { time, m } = model();
    ended(m, time.now, "released");
    time.advance(60_000);
    m.onTick(time.now);
    // Nothing is shown for these reasons (the screen maps them to no line); the value stays.
    expect(m.state.talkEnd).toBe("released");
    ended(m, time.now, "stopped");
    time.advance(60_000);
    m.onTick(time.now);
    expect(m.state.talkEnd).toBe("stopped");
  });

  it("keeps 'may not be audible' until the next press", () => {
    const { time, m } = model();
    m.onEvent({ type: "talk_playback", result: "unconfirmed" }, time.now);
    ended(m, time.now, "released");
    time.advance(10 * 60_000);
    m.onTick(time.now);
    expect(m.state.talkUnconfirmed).toBe(true);
    m.onEvent({ type: "talk_changed", state: { state: "requesting" } }, time.now);
    expect(m.state.talkUnconfirmed).toBe(false);
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
