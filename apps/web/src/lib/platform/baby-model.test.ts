import type { LinkStatus } from "@guardedsleep/core-wasm";
import { describe, expect, it } from "vitest";
import type { BabyMonitorState } from "../contract";
import { BabyModel } from "./baby-model";
import { FakeTime } from "./test-support";

const status = (state: LinkStatus["state"], over: Partial<LinkStatus> = {}): LinkStatus => ({
  state,
  level: 0,
  key: null,
  cause: null,
  acknowledged_until: null,
  last_alive_ms: null,
  ...over,
});

function setup() {
  const time = new FakeTime();
  let state: BabyMonitorState | null = null;
  const model = new BabyModel(time, (s) => (state = s));
  return {
    time,
    model,
    get state(): BabyMonitorState {
      if (!state) throw new Error("nothing published yet");
      return state;
    },
  };
}

describe("BabyModel", () => {
  const phases: Array<[LinkStatus, Record<string, unknown>]> = [
    [
      status("connecting", { level: 2, cause: "server_unreachable" }),
      { phase: "connecting", level: 2, causeHint: "server_unreachable" },
    ],
    [status("connected", { level: 3 }), { phase: "connected", level: 0 }],
    [
      status("reconnecting", { level: 1, last_alive_ms: 40 }),
      { phase: "reconnecting", lastAliveElapsedMs: 40 },
    ],
    [status("peer_stopped"), { phase: "peer_stopped" }],
    [
      status("failed_needs_user", { key: "update_required" }),
      { phase: "failed_needs_user", failureKey: "update_required" },
    ],
    [status("stopped"), { phase: "stopped" }],
  ];
  it.each(phases)("maps %j", (input, expected) => {
    const t = setup();
    t.model.onStatus(input);
    expect(t.state.link).toMatchObject(expected);
    expect(t.state.link.quality).toBe("unknown");
  });

  it("shows the microphone level clamped to 0..1", () => {
    const t = setup();
    t.model.onMicLevel(0.4, 0);
    expect(t.state.micLevel).toBeCloseTo(0.4);
    t.model.onMicLevel(3, 100);
    expect(t.state.micLevel).toBe(1);
  });

  it("flags a microphone that delivers only digital silence for 30 s", () => {
    const t = setup();
    t.model.onMicLevel(0, 0);
    t.model.onMicLevel(0, 29_000);
    expect(t.state.micSilent).toBe(false);
    t.model.onMicLevel(0, 30_000);
    expect(t.state.micSilent).toBe(true);
    t.model.onMicLevel(0.01, 31_000);
    expect(t.state.micSilent).toBe(false);
  });

  it("takes the live noise gate from the core's echo", () => {
    const t = setup();
    t.model.onEvent({ type: "noise_gate_changed", threshold: 0.3, hold_ms: 7_000 });
    expect(t.state).toMatchObject({ threshold: 0.3, holdSeconds: 7 });
  });

  it("shows when the parent listens, from the core's listen echo", () => {
    const t = setup();
    t.model.onEvent({ type: "listen_changed", enabled: true });
    expect(t.state.parentListening).toBe(true);
    t.model.onEvent({ type: "listen_changed", enabled: false });
    expect(t.state.parentListening).toBe(false);
  });

  it("ignores events that only matter to the parent", () => {
    const t = setup();
    t.model.setSettings(0.12, 5);
    const before = t.state;
    t.model.onEvent({ type: "peer_stopped" });
    t.model.onEvent({ type: "link_lost", down_ms: 1 });
    expect(t.state).toEqual(before);
  });

  it("keeps the gate settings across a reset and clears the rest", () => {
    const t = setup();
    t.model.setSettings(0.4, 9);
    t.model.setKioskLocked(true);
    t.model.onStatus(status("connected"));
    t.model.reset();
    expect(t.state).toMatchObject({ threshold: 0.4, holdSeconds: 9, kioskLocked: false });
    expect(t.state.link.phase).toBe("idle");
  });

  it("a run that cannot start is a terminal failure with a web-only key", () => {
    const t = setup();
    t.model.fail("mic_denied");
    expect(t.state.link).toMatchObject({ phase: "failed_needs_user", failureKey: "mic_denied" });
  });

  it("kiosk lock and the restored-notice flag are plain toggles", () => {
    const t = setup();
    t.model.setKioskLocked(true);
    expect(t.state.kioskLocked).toBe(true);
    t.model.setSilenceRestoredNotice(true);
    expect(t.state.silenceRestoredNotice).toBe(true);
    t.model.setSilenceRestoredNotice(false);
    expect(t.state.silenceRestoredNotice).toBe(false);
  });
});
