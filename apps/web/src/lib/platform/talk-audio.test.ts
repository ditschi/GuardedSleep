import { describe, expect, it } from "vitest";
import { createSoundGate } from "./audio";
import type { PeerConnectionCtor } from "./attempt";
import {
  browserTalkSupported,
  createTalkPlayback,
  microphonePermission,
  openTalkCapture,
  requestMicrophone,
} from "./talk-audio";
import { TalkRtpWatch } from "./talk-rtp-watch";
import { FakeAttempt, FakeTime } from "./test-support";
import {
  asContext,
  asStream,
  FakeAudioContext,
  FakeStream,
  FakeTrack,
  type FakeNode,
} from "./test-audio";
import { TALK_LEVEL_GAIN, TALK_LIMITER_CEILING } from "./policy";
import { createWebMediaEngine } from "./web-media";

function makePlayback() {
  const ctx = new FakeAudioContext();
  ctx.state = "running";
  const time = new FakeTime();
  const sinks: Array<{ srcObject: unknown; muted: boolean; played: boolean }> = [];
  const playback = createTalkPlayback({
    gate: createSoundGate(() => asContext(ctx)),
    timers: time,
    createSink: () => {
      const sink = {
        srcObject: null as MediaProvider | null,
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
  const track = new FakeTrack();
  const stream = new FakeStream([track]);
  playback.attach(track as unknown as MediaStreamTrack, asStream(stream));
  // The output stage: level -> compressor -> shaper -> (lease) -> destination.
  const [shaper] = ctx.shapers;
  const [compressor] = ctx.compressors;
  return { ctx, time, sinks, playback, track, shaper, compressor };
}

const reachesDestination = (ctx: FakeAudioContext, from: FakeNode): boolean =>
  from.to.includes(ctx.destination);

describe("baby talk playback", () => {
  it("has no path to the loudspeaker until a lease opens, and keeps the element muted", () => {
    const { ctx, sinks, shaper, track } = makePlayback();
    expect(reachesDestination(ctx, shaper!)).toBe(false);
    expect(ctx.destination.to).toEqual([]);
    expect(track.enabled).toBe(false);
    // No unmuted element ever holds a talk stream.
    expect(sinks).toHaveLength(1);
    expect(sinks.every((s) => s.muted && s.played)).toBe(true);
  });

  it("opens only inside the lease and fades in to the level's gain", () => {
    const { ctx, playback, shaper, track } = makePlayback();
    playback.open("medium", 800, 12_000);
    expect(reachesDestination(ctx, shaper!)).toBe(true);
    expect(track.enabled).toBe(true);
    expect(playback.isOpen).toBe(true);
    const level = ctx.gains[0]!;
    expect(level.gain.ramps).toContainEqual(["ramp", TALK_LEVEL_GAIN.medium, 0.8]);
  });

  it("schedules the end on the audio clock at grant time", () => {
    const { ctx, playback } = makePlayback();
    ctx.currentTime = 5;
    playback.open("low", 800, 12_000);
    // 12 s after the grant on the audio clock, whatever the page's timers do.
    expect(ctx.gains[0]!.gain.ramps).toContainEqual(["set", 0, 17]);
  });

  it("disconnects at the lease deadline by a platform timer too", () => {
    const { ctx, time, playback, shaper, track } = makePlayback();
    playback.open("low", 800, 3_000);
    time.advance(2_999);
    expect(playback.isOpen).toBe(true);
    time.advance(1);
    expect(playback.isOpen).toBe(false);
    expect(reachesDestination(ctx, shaper!)).toBe(false);
    expect(track.enabled).toBe(false);
  });

  it("close ends the lease at once and is idempotent", () => {
    const { ctx, playback, shaper } = makePlayback();
    playback.open("low", 800, 20_000);
    playback.close();
    playback.close();
    expect(reachesDestination(ctx, shaper!)).toBe(false);
    expect(playback.isOpen).toBe(false);
  });

  it("puts a high-ratio fast limiter and a clip in the chain", () => {
    const { ctx, compressor, shaper } = makePlayback();
    const level = ctx.gains[0]!;
    expect(level.to).toContain(compressor);
    expect(compressor!.to).toContain(shaper);
    expect(compressor!.ratio.value).toBeGreaterThanOrEqual(12);
    expect(compressor!.attack.value).toBeLessThanOrEqual(0.003);
    const curve = shaper!.curve!;
    expect(Math.max(...curve)).toBeCloseTo(TALK_LIMITER_CEILING, 6);
    expect(Math.min(...curve)).toBeCloseTo(-TALK_LIMITER_CEILING, 6);
  });

  it("plays a generated chime through the same limiter and reports its end", () => {
    const { ctx, time, playback, shaper } = makePlayback();
    let done = 0;
    playback.playChime("low", () => done++);
    expect(ctx.oscillators.length).toBe(2);
    expect(ctx.oscillators.every((o) => o.started !== null)).toBe(true);
    expect(reachesDestination(ctx, shaper!)).toBe(true);
    // The voice stays closed while the chime plays.
    expect(playback.isOpen).toBe(false);
    time.advance(600);
    expect(done).toBe(1);
    expect(reachesDestination(ctx, shaper!)).toBe(false);
  });

  it("a cancelled chime never reports its end and leaves nothing connected", () => {
    const { ctx, playback, time, shaper } = makePlayback();
    let done = 0;
    playback.playChime("high", () => done++);
    playback.close();
    time.advance(2_000);
    expect(done).toBe(0);
    expect(reachesDestination(ctx, shaper!)).toBe(false);
  });

  it("measures the incoming voice only while a lease is open", () => {
    const { ctx, time, playback } = makePlayback();
    ctx.analysers[0]!.samples = [0.2, -0.2];
    time.advance(20);
    expect(playback.drainRms()).toBeNull();
    playback.open("low", 800, 10_000);
    time.advance(20);
    expect(playback.drainRms()).toBeCloseTo(0.2, 6);
  });

  it("release tears the whole graph down", () => {
    const { time, playback, track } = makePlayback();
    playback.open("low", 800, 10_000);
    playback.release();
    expect(time.pending()).toBe(0);
    expect(track.enabled).toBe(false);
  });
});

describe("parent talk capture", () => {
  it("asks for voice processing and no video, and hands over the bare track", async () => {
    const asked: MediaStreamConstraints[] = [];
    const stream = new FakeStream();
    const capture = openTalkCapture((c) => {
      asked.push(c);
      return Promise.resolve(asStream(stream));
    });
    expect(asked[0]).toEqual({
      audio: {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
        channelCount: 1,
      },
      video: false,
    });
    expect(await capture.ready).toBe(stream.tracks[0]);
  });

  it("stop closes the microphone, also when the browser answers after the stop", async () => {
    const early = new FakeStream();
    const a = openTalkCapture(() => Promise.resolve(asStream(early)));
    await a.ready;
    a.stop();
    expect(early.tracks.every((t) => t.stopped)).toBe(true);

    const late = new FakeStream();
    let resolve: (s: MediaStream) => void = () => undefined;
    const b = openTalkCapture(() => new Promise<MediaStream>((r) => (resolve = r)));
    b.stop();
    resolve(asStream(late));
    expect(await b.ready).toBeNull();
    expect(late.tracks.every((t) => t.stopped)).toBe(true);
  });

  it("reports a refused microphone as no track", async () => {
    const c = openTalkCapture(() => Promise.reject(new DOMException("no", "NotAllowedError")));
    expect(await c.ready).toBeNull();
  });
});

describe("microphone permission step", () => {
  it("closes the microphone again right after asking", async () => {
    const stream = new FakeStream();
    expect(await requestMicrophone(() => Promise.resolve(asStream(stream)))).toBe("granted");
    expect(stream.tracks.every((t) => t.stopped)).toBe(true);
  });

  it("reports a refusal", async () => {
    expect(
      await requestMicrophone(() => Promise.reject(new DOMException("no", "NotAllowedError"))),
    ).toBe("refused");
  });

  it("reads the Permissions API where it exists", async () => {
    expect(
      await microphonePermission({
        permissions: { query: () => Promise.resolve({ state: "granted" }) },
      }),
    ).toBe("granted");
    expect(
      await microphonePermission({
        permissions: { query: () => Promise.reject(new TypeError("x")) },
      }),
    ).toBe("unknown");
    expect(await microphonePermission({})).toBe("unknown");
  });

  it("reports talk-back as unsupported on iOS and without media devices", () => {
    const ua = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Safari";
    expect(browserTalkSupported({ userAgent: ua, mediaDevices: {} })).toBe(false);
    expect(
      browserTalkSupported({
        userAgent: "Mozilla/5.0 (Macintosh)",
        platform: "MacIntel",
        maxTouchPoints: 5,
        mediaDevices: {},
      }),
    ).toBe(false);
    expect(
      browserTalkSupported({ userAgent: "Mozilla/5.0 (X11; Linux) Chrome", mediaDevices: {} }),
    ).toBe(true);
    expect(browserTalkSupported({ userAgent: "Chrome" })).toBe(false);
  });
});

describe("TalkRtpWatch", () => {
  it("reports packets outside a lease, not inside one or just after", () => {
    const w = new TalkRtpWatch();
    expect(w.observe(0, false, 0)).toBe(false);
    expect(w.observe(0, false, 1_000)).toBe(false);
    expect(w.observe(50, true, 2_000)).toBe(false);
    // The parent's last frames after the lease closed are inside the grace.
    expect(w.observe(60, false, 3_000)).toBe(false);
    expect(w.observe(70, false, 3_500)).toBe(false);
    expect(w.observe(80, false, 6_000)).toBe(true);
    expect(w.observe(80, false, 7_000)).toBe(false);
  });

  it("reports packets on a line that never had a lease", () => {
    const w = new TalkRtpWatch();
    w.observe(0, false, 0);
    expect(w.observe(5, false, 1_000)).toBe(true);
  });
});

describe("parent talk microphone on the engine", () => {
  async function engine() {
    const ctx = new FakeAudioContext();
    const time = new FakeTime();
    const opened: FakeStream[] = [];
    let calls = 0;
    const media = await createWebMediaEngine("parent", {
      PeerConnection: class {} as unknown as PeerConnectionCtor,
      userIceServers: [],
      timers: time,
      gate: createSoundGate(() => asContext(ctx)),
      getUserMedia: () => {
        calls++;
        const s = new FakeStream();
        opened.push(s);
        return Promise.resolve(asStream(s));
      },
      createSink: () => ({ srcObject: null, muted: false, play: () => Promise.resolve() }),
      createStream: () => asStream(new FakeStream()),
    });
    const port = new FakeAttempt(false, {} as never);
    return { media, port, opened, calls: () => calls };
  }

  it("opens no microphone and sends no track outside a talk", async () => {
    const { port, calls } = await engine();
    expect(calls()).toBe(0);
    expect(port.talkTracks).toEqual([]);
  });

  it("attaches the warm capture at the grant and removes and stops it on release", async () => {
    const { media, port, opened } = await engine();
    expect(await media.warmTalkMic()).toBe(true);
    expect(port.talkTracks).toEqual([]);
    expect(await media.setTalkMic(port, true)).toBe("attached");
    expect(port.talkTracks).toEqual([opened[0]!.tracks[0]]);
    expect(await media.setTalkMic(port, false)).toBe("detached");
    expect(port.talkTracks.at(-1)).toBeNull();
    expect(opened[0]!.tracks.every((t) => t.stopped)).toBe(true);
  });

  it("stops a warm capture that was never attached (released before the grant)", async () => {
    const { media, opened } = await engine();
    await media.warmTalkMic();
    media.discardTalkMic();
    expect(opened[0]!.tracks.every((t) => t.stopped)).toBe(true);
  });

  it("a refused microphone fails the attach", async () => {
    const ctx = new FakeAudioContext();
    const media = await createWebMediaEngine("parent", {
      PeerConnection: class {} as unknown as PeerConnectionCtor,
      userIceServers: [],
      timers: new FakeTime(),
      gate: createSoundGate(() => asContext(ctx)),
      getUserMedia: () => Promise.reject(new DOMException("no", "NotAllowedError")),
      createSink: () => ({ srcObject: null, muted: false, play: () => Promise.resolve() }),
      createStream: () => asStream(new FakeStream()),
    });
    const port = new FakeAttempt(false, {} as never);
    expect(await media.setTalkMic(port, true)).toBe("failed");
    expect(port.talkTracks).toEqual([]);
  });

  it("a release that overtakes the attach makes it stale and leaves no microphone open", async () => {
    const { media, port, opened } = await engine();
    const attach = media.setTalkMic(port, true);
    const detach = media.setTalkMic(port, false);
    expect(await attach).toBe("stale");
    expect(await detach).toBe("detached");
    expect(opened[0]!.tracks.every((t) => t.stopped)).toBe(true);
    expect(port.talkTracks.filter((t) => t !== null)).toEqual([]);
  });
});

// Nothing on the talk path may record: no MediaRecorder, no extra sink, no worklet, no AEC dump.
const talkSources = import.meta.glob(["./talk-audio.ts", "./web-media.ts", "./attempt.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

describe("no recording on the talk path", () => {
  it("scans the talk sources", () => {
    expect(Object.keys(talkSources)).toHaveLength(3);
  });

  it("uses no MediaRecorder, addSink, worklet, AEC dump or event log", () => {
    for (const [file, source] of Object.entries(talkSources)) {
      const code = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/\/\/.*$/gm, "");
      expect(code, file).not.toMatch(/MediaRecorder|addSink|AudioWorklet|startAecDump|RtcEventLog/);
    }
  });
});
