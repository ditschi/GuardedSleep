import type { LinkEvent, LinkState, LinkStatus } from "@guardedsleep/core-wasm";
import { describe, expect, it } from "vitest";
import type { AlertSink, LinkSnapshot, NoiseKind, Notice } from "./alert-sink";
import { noisePlaysAudio } from "./noise-feedback";
import { ParentModel, type ParentEffects } from "./parent-model";
import { FakeTime } from "./test-support";

const status = (state: LinkState, level = 0): LinkStatus => ({
  state,
  level,
  key: null,
  cause: null,
  acknowledged_until: null,
  last_alive_ms: null,
});

class Sink implements AlertSink {
  noise: Array<[NoiseKind, boolean, boolean, boolean]> = [];
  snapshots: LinkSnapshot[] = [];
  ended = 0;
  linkState(s: LinkSnapshot): void {
    this.snapshots.push(s);
  }
  linkLost(): void {}
  noiseAlert(kind: NoiseKind, probe: boolean, snoozed: boolean, discreet: boolean): void {
    this.noise.push([kind, probe, snoozed, discreet]);
  }
  noiseEnded(): void {
    this.ended++;
  }
  stopNoiseSound(): void {}
  notice(_n: Notice): void {}
  clear(): void {}
}

class Effects implements ParentEffects {
  muted: boolean[] = [];
  snoozeEnded_ = 0;
  muteRemote(muted: boolean): void {
    this.muted.push(muted);
  }
  listenStopped(): void {}
  testAlertArrived(): void {}
  snoozeEnded(): void {
    this.snoozeEnded_++;
  }
}

function setup(initial: boolean) {
  const time = new FakeTime();
  const sink = new Sink();
  const effects = new Effects();
  let discreet = initial;
  const model = new ParentModel(
    time,
    sink,
    effects,
    () => 0.12,
    () => undefined,
    () => discreet,
  );
  const now = (): number => time.elapsedMs();
  const noise = (snoozed = false, probe = false): void =>
    model.onEvent({ type: "noise_alert", rms: 0.4, probe, snoozed } as LinkEvent, now());
  model.onStatus(status("connected"), now());
  return {
    time,
    sink,
    effects,
    model,
    noise,
    now,
    setDiscreet: (v: boolean) => (discreet = v),
  };
}

describe("alert style discreet", () => {
  it("plays audio only for unsnoozed noise in the sound style", () => {
    expect(noisePlaysAudio(false, false)).toBe(true);
    expect(noisePlaysAudio(false, true)).toBe(false);
    expect(noisePlaysAudio(true, false)).toBe(false);
    expect(noisePlaysAudio(true, true)).toBe(false);
  });

  it("alerts but plays nothing until Listen", () => {
    const t = setup(true);
    t.noise();
    expect(t.sink.noise.at(-1)).toEqual(["new", false, false, true]);
    expect(t.effects.muted.at(-1)).toBe(true);
    expect(t.model.state.noiseAlertActive).toBe(true);
    expect(t.model.setListen("hold", t.now())).toBe(true);
    expect(t.effects.muted.at(-1)).toBe(false);
    t.time.advance(10_000);
    t.noise();
    expect(t.effects.muted.at(-1)).toBe(false);
    t.model.setListen("off", t.now());
    expect(t.effects.muted.at(-1)).toBe(true);
  });

  it("the sound style still plays the audio", () => {
    const t = setup(false);
    t.noise();
    expect(t.effects.muted.at(-1)).toBe(false);
    t.model.setListen("hold", t.now());
    t.model.setListen("off", t.now());
    expect(t.effects.muted.at(-1)).toBe(false);
  });

  it("switching to discreet mutes a running event unless listening", () => {
    const t = setup(false);
    t.noise();
    t.setDiscreet(true);
    t.model.onAlertStyleChanged();
    expect(t.effects.muted.at(-1)).toBe(true);
    t.model.setListen("locked", t.now());
    t.model.onAlertStyleChanged();
    expect(t.effects.muted.at(-1)).toBe(false);
  });

  it("a snooze that ends during noise alerts fully but stays silent when discreet", () => {
    const t = setup(true);
    t.model.snooze(10);
    t.noise(true);
    t.time.advance(10 * 60_000);
    t.noise(true);
    t.model.onTick(t.now());
    expect(t.effects.snoozeEnded_).toBe(1);
    expect(t.sink.noise.at(-1)).toEqual(["new", false, false, true]);
    expect(t.effects.muted.at(-1)).toBe(true);
  });

  it("a test alert runs the active style", () => {
    const t = setup(true);
    t.noise(false, true);
    expect(t.sink.noise.at(-1)).toEqual(["new", true, false, true]);
    expect(t.model.state.noiseIsTest).toBe(true);
    expect(t.effects.muted.at(-1)).toBe(true);
  });

  it("connection levels do not depend on the style", () => {
    const t = setup(true);
    t.model.onStatus(status("reconnecting", 3), t.now());
    expect(t.sink.snapshots.at(-1)).toMatchObject({ phase: "reconnecting", level: 3 });
    expect(t.effects.muted).toEqual([]);
  });
});
