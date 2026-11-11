import { describe, expect, it } from "vitest";
import {
  AlertPlayer,
  createRemotePlayback,
  createSoundGate,
  openMicCapture,
  RmsMeter,
  tonePlanForLink,
} from "./audio";
import type { LinkSnapshot } from "./alert-sink";
import type { PeerConnectionCtor } from "./attempt";
import { FakeTime } from "./test-support";
import { asContext, asStream, FakeAudioContext, FakeStream, type FakeAnalyser } from "./test-audio";
import { createWebMediaEngine } from "./web-media";

function gateOf(ctx: FakeAudioContext) {
  return createSoundGate(() => asContext(ctx));
}

const snapshot = (over: Partial<LinkSnapshot>): LinkSnapshot => ({
  phase: "reconnecting",
  level: 0,
  acknowledged: false,
  causeHint: null,
  wasConnected: true,
  ...over,
});

describe("RmsMeter", () => {
  it("averages the squares of everything sampled since the last drain", () => {
    const ctx = new FakeAudioContext();
    const analyser = ctx.createAnalyser();
    const time = new FakeTime();
    const meter = new RmsMeter(analyser as unknown as AnalyserNode, time);
    analyser.samples = [0.5, -0.5];
    time.advance(20);
    analyser.samples = [0, 0];
    time.advance(20);
    // two windows: one at 0.5, one silent -> sqrt(0.125)
    expect(meter.drain()).toBeCloseTo(Math.sqrt(0.125), 6);
    expect(meter.drain()).toBeNull();
    meter.stop();
    time.advance(100);
    expect(meter.drain()).toBeNull();
  });
});

describe("SoundGate", () => {
  it("is locked until a gesture resumes the context", async () => {
    const ctx = new FakeAudioContext();
    const gate = gateOf(ctx);
    const seen: boolean[] = [];
    gate.onChange((u) => seen.push(u));
    expect(gate.unlocked).toBe(false);
    expect(await gate.unlock()).toBe(true);
    expect(gate.unlocked).toBe(true);
    expect(seen).toEqual([true]);
  });

  it("stays locked when the browser refuses to resume", async () => {
    const ctx = new FakeAudioContext();
    ctx.allowResume = false;
    const gate = gateOf(ctx);
    expect(await gate.unlock()).toBe(false);
    expect(gate.unlocked).toBe(false);
  });

  it("locks again when the context is suspended by the system", async () => {
    const ctx = new FakeAudioContext();
    const gate = gateOf(ctx);
    await gate.unlock();
    ctx.setState("suspended");
    expect(gate.unlocked).toBe(false);
    ctx.setState("running");
    expect(gate.unlocked).toBe(true);
  });
});

describe("baby microphone", () => {
  async function open() {
    const ctx = new FakeAudioContext();
    const gate = gateOf(ctx);
    const time = new FakeTime();
    const mic = new FakeStream();
    const requested: MediaStreamConstraints[] = [];
    const capture = await openMicCapture({
      getUserMedia: (c) => {
        requested.push(c);
        return Promise.resolve(asStream(mic));
      },
      gate,
      timers: time,
    });
    return { ctx, time, mic, capture, requested };
  }

  it("asks for the raw microphone without processing and no video", async () => {
    const { requested } = await open();
    expect(requested[0]).toEqual({
      audio: {
        echoCancellation: false,
        noiseSuppression: false,
        autoGainControl: false,
        channelCount: 1,
      },
      video: false,
    });
  });

  it("sends nothing until the core opens the gate", async () => {
    const { ctx, capture } = await open();
    const gain = ctx.gains[0];
    expect(gain?.gain.value).toBe(0);
    capture.setEnabled(true);
    expect(gain?.gain.value).toBe(1);
    capture.setEnabled(false);
    expect(gain?.gain.value).toBe(0);
  });

  it("measures the real microphone, not the gate output", async () => {
    const { ctx, time, capture } = await open();
    const analyser = ctx.analysers[0] as FakeAnalyser;
    analyser.samples = [0.3, -0.3];
    time.advance(20);
    expect(capture.drainRms()).toBeCloseTo(0.3, 6);
    // The analyser hangs off the source, in parallel to the gate.
    expect(ctx.sources[0]?.to).toContain(analyser);
  });

  it("mixes the probe tone into the outgoing stream only, and ends it", async () => {
    const { ctx, time, capture } = await open();
    capture.playProbeTone(1500);
    const osc = ctx.oscillators[0];
    expect(osc?.frequency.value).toBe(1000);
    expect(osc?.started).not.toBeNull();
    const level = osc?.to[0];
    expect(level?.to).toEqual([ctx.destinations[0]]);
    expect(ctx.destination.to).toEqual([]);
    time.advance(1499);
    expect(osc?.stopped).toBeNull();
    time.advance(1);
    expect(osc?.stopped).not.toBeNull();
  });

  it("release stops the microphone tracks and the meter", async () => {
    const { time, mic, capture } = await open();
    capture.release();
    expect(mic.tracks.every((t) => t.stopped)).toBe(true);
    expect(time.pending()).toBe(0);
  });

  it("rejects when the permission is refused", async () => {
    const ctx = new FakeAudioContext();
    await expect(
      openMicCapture({
        getUserMedia: () => Promise.reject(new DOMException("denied", "NotAllowedError")),
        gate: gateOf(ctx),
        timers: new FakeTime(),
      }),
    ).rejects.toThrow();
  });
});

describe("parent playback", () => {
  function make() {
    const ctx = new FakeAudioContext();
    const gate = gateOf(ctx);
    const time = new FakeTime();
    const sinks: Array<{ srcObject: unknown; muted: boolean; played: boolean }> = [];
    const playback = createRemotePlayback({
      gate,
      timers: time,
      createSink: () => {
        const sink = {
          srcObject: null as MediaStream | null,
          muted: false,
          played: false,
          play() {
            this.played = true;
            return Promise.resolve();
          },
        };
        sinks.push(sink);
        return sink;
      },
    });
    return { ctx, time, sinks, playback, gate };
  }

  it("ducks only the baby's audio and keeps the meter live, apart from the mute", () => {
    const { ctx, playback, time } = make();
    playback.play(asStream(new FakeStream()));
    const [mute, duck] = ctx.gains;
    playback.setDucked(true);
    expect(duck?.gain.value).toBe(0);
    expect(mute?.gain.value).toBe(1);
    // The analyser sits in front of the duck: the level meter stays live while talking.
    expect(ctx.analysers[0]?.to).toContain(duck);
    ctx.analysers[0]!.samples = [0.2, -0.2];
    time.advance(20);
    expect(playback.drainRms()).toBeCloseTo(0.2, 6);
    playback.setMuted(true);
    playback.setDucked(false);
    expect(mute?.gain.value).toBe(0);
    expect(duck?.gain.value).toBe(1);
  });

  it("plays through Web Audio and keeps the stream alive in a muted element", () => {
    const { ctx, sinks, playback, time } = make();
    const stream = new FakeStream();
    playback.play(asStream(stream));
    expect(sinks[0]).toMatchObject({ muted: true, played: true, srcObject: stream });
    expect(ctx.sources[0]?.to).toContain(ctx.analysers[0]);
    ctx.analysers[0]!.samples = [0.2, -0.2];
    time.advance(20);
    expect(playback.drainRms()).toBeCloseTo(0.2, 6);
  });

  it("replaces the previous attempt's stream", () => {
    const { ctx, sinks, playback } = make();
    playback.play(asStream(new FakeStream()));
    playback.play(asStream(new FakeStream()));
    expect(ctx.sources[0]?.disconnected).toBe(true);
    expect(sinks[0]?.srcObject).toBeNull();
    expect(sinks).toHaveLength(2);
  });

  it("mutes without stopping the meter and clamps the gain", () => {
    const { ctx, playback, gate } = make();
    playback.play(asStream(new FakeStream()));
    playback.setMuted(true);
    const mute = ctx.gains[0];
    expect(mute?.gain.value).toBe(0);
    playback.setMuted(false);
    expect(mute?.gain.value).toBe(1);
    playback.setGainPct(300);
    expect(gate.master().gain.value).toBe(1.5);
    playback.setGainPct(5);
    expect(gate.master().gain.value).toBe(0.25);
    playback.setGainPct(100);
    expect(gate.master().gain.value).toBe(1);
  });
});

describe("tonePlanForLink", () => {
  const cases: Array<[string, Partial<LinkSnapshot>, ReturnType<typeof tonePlanForLink>]> = [
    ["connected is silent", { phase: "connected" }, { toneEveryMs: null, continuousAlarm: false }],
    ["level 1 is visual only", { level: 1 }, { toneEveryMs: null, continuousAlarm: false }],
    ["level 2 repeats a soft tone", { level: 2 }, { toneEveryMs: 10_000, continuousAlarm: false }],
    ["level 3 loops the alarm", { level: 3 }, { toneEveryMs: null, continuousAlarm: true }],
    [
      "I'm checking mutes level 2",
      { level: 2, acknowledged: true },
      { toneEveryMs: null, continuousAlarm: false },
    ],
    [
      "I'm checking mutes level 3",
      { level: 3, acknowledged: true },
      { toneEveryMs: null, continuousAlarm: false },
    ],
    [
      "a failure after a working link is never silent",
      { phase: "failed_needs_user", level: 0 },
      { toneEveryMs: 10_000, continuousAlarm: false },
    ],
    [
      "a failure of a link that never worked stays quiet",
      { phase: "failed_needs_user", wasConnected: false },
      { toneEveryMs: null, continuousAlarm: false },
    ],
    [
      "a stopped baby device has no loop",
      { phase: "peer_stopped", level: 3 },
      { toneEveryMs: null, continuousAlarm: false },
    ],
  ];
  it.each(cases)("%s", (_name, over, expected) => {
    expect(tonePlanForLink(snapshot(over))).toEqual(expected);
  });
});

describe("AlertPlayer", () => {
  function make() {
    const ctx = new FakeAudioContext();
    const gate = gateOf(ctx);
    const time = new FakeTime();
    const player = new AlertPlayer({ gate, timers: time });
    return { ctx, gate, time, player };
  }
  const freqs = (ctx: FakeAudioContext): number[] => ctx.oscillators.map((o) => o.frequency.value);

  it("makes no sound before the unlock gesture", () => {
    const { ctx, time, player } = make();
    player.linkState(snapshot({ level: 3 }));
    player.noiseAlert("new", true, false);
    player.notice({ type: "peer_stopped" });
    time.advance(30_000);
    expect(ctx.oscillators).toHaveLength(0);
  });

  it("level 2: a soft tone at once and every 10 s, nothing after the link returns", async () => {
    const { ctx, gate, time, player } = make();
    await gate.unlock();
    player.linkState(snapshot({ level: 2 }));
    expect(freqs(ctx)).toEqual([660]);
    time.advance(10_000);
    expect(freqs(ctx)).toEqual([660, 660]);
    player.linkState(snapshot({ phase: "connected", level: 0 }));
    time.advance(30_000);
    expect(ctx.oscillators).toHaveLength(2);
    expect(time.pending()).toBe(0);
  });

  it("level 3: the two-tone alarm loops until acknowledged", async () => {
    const { ctx, gate, time, player } = make();
    await gate.unlock();
    player.linkState(snapshot({ level: 3 }));
    expect(freqs(ctx)).toEqual([880, 660]);
    time.advance(1_200);
    expect(freqs(ctx)).toEqual([880, 660, 880, 660]);
    player.linkState(snapshot({ level: 3, acknowledged: true }));
    time.advance(5_000);
    expect(ctx.oscillators).toHaveLength(4);
    expect(time.pending()).toBe(0);
  });

  it("does not restart a loop for an unchanged plan", async () => {
    const { ctx, gate, player } = make();
    await gate.unlock();
    player.linkState(snapshot({ level: 2 }));
    player.linkState(snapshot({ level: 2 }));
    expect(ctx.oscillators).toHaveLength(1);
  });

  it("noise adds no tone (the baby's audio is the alert); a test alert plays the test tone", async () => {
    const { ctx, gate, player } = make();
    await gate.unlock();
    player.noiseAlert("new", false, false);
    player.noiseAlert("continue", false, false);
    expect(ctx.oscillators).toHaveLength(0);
    player.noiseAlert("new", true, false);
    expect(freqs(ctx)).toEqual([1000]);
  });

  it("a stopped baby device gets one soft tone, no loop", async () => {
    const { ctx, gate, time, player } = make();
    await gate.unlock();
    player.notice({ type: "peer_stopped" });
    time.advance(60_000);
    expect(freqs(ctx)).toEqual([660]);
  });

  it("clear cancels every loop", async () => {
    const { gate, time, player } = make();
    await gate.unlock();
    player.linkState(snapshot({ level: 3 }));
    player.clear();
    expect(time.pending()).toBe(0);
  });
});

describe("web media engine", () => {
  class MiniPc {
    transceivers: unknown[] = [];
    static lastConfig: RTCConfiguration | null = null;
    ondatachannel = null;
    constructor(config: RTCConfiguration) {
      MiniPc.lastConfig = config;
    }
    addTransceiver(track: unknown, init: unknown): void {
      this.transceivers.push([track, init]);
    }
    createDataChannel(): object {
      return { readyState: "connecting" };
    }
  }

  const base = (ctx: FakeAudioContext, getUserMedia: () => Promise<MediaStream>) => ({
    PeerConnection: MiniPc as unknown as PeerConnectionCtor,
    userIceServers: [{ urls: ["turn:own.test:3478"], username: "u", credential: "p" }],
    timers: new FakeTime(),
    gate: gateOf(ctx),
    getUserMedia,
    createSink: () => ({ srcObject: null, muted: false, play: () => Promise.resolve() }),
    createStream: () => asStream(new FakeStream()),
  });

  it("baby: opens the microphone and sends it sendonly with the core ICE list and the own TURN", async () => {
    const ctx = new FakeAudioContext();
    const engine = await createWebMediaEngine(
      "baby",
      base(ctx, () => Promise.resolve(asStream(new FakeStream()))),
    );
    engine.createAttempt(
      true,
      {
        onLocalDescription() {},
        onLocalIce() {},
        onDataOpen() {},
        onDataMessage() {},
        onDataClosed() {},
        onConnectionState() {},
        onDtlsFingerprint() {},
      },
      [
        { urls: ["stun:gs.test:3478"] },
        { urls: ["turn:gs.test:3478"], username: "1700000000:x", credential: "secret" },
      ],
    );
    // The core's entries unchanged and in order, the user's TURN after them.
    expect(MiniPc.lastConfig).toEqual({
      iceServers: [
        { urls: ["stun:gs.test:3478"] },
        { urls: ["turn:gs.test:3478"], username: "1700000000:x", credential: "secret" },
        { urls: ["turn:own.test:3478"], username: "u", credential: "p" },
      ],
    });
    engine.setMicEnabled(true);
    expect(ctx.gains[0]?.gain.value).toBe(1);
    expect(engine.drainRemoteRms()).toBeNull();
    engine.release();
  });

  it("baby: a refused microphone rejects the engine", async () => {
    const ctx = new FakeAudioContext();
    await expect(
      createWebMediaEngine(
        "baby",
        base(ctx, () => Promise.reject(new Error("denied"))),
      ),
    ).rejects.toThrow("denied");
  });

  it("parent: builds the playout chain and has no microphone", async () => {
    const ctx = new FakeAudioContext();
    const engine = await createWebMediaEngine(
      "parent",
      base(ctx, () => Promise.reject(new Error("must not be called"))),
    );
    engine.setPlaybackMuted(true);
    expect(ctx.gains[0]?.gain.value).toBe(0);
    expect(engine.drainMicRms()).toBeNull();
    engine.startProbeTone(100);
    expect(ctx.oscillators).toHaveLength(0);
  });
});
