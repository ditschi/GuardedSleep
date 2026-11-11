import type { IceCandidate } from "@guardedsleep/core-wasm";
import { describe, expect, it } from "vitest";
import { createRtcAttempt, remoteFingerprint, type PeerConnectionCtor } from "./attempt";
import type { AttemptListener } from "./media-engine";
import { FINGERPRINT_RETRY_MS } from "./policy";
import { FakeTime } from "./test-support";

type Stats = Record<string, unknown>;

function report(entries: Record<string, Stats>): RTCStatsReport {
  const map = new Map(Object.entries(entries));
  return map as unknown as RTCStatsReport;
}

const WITH_CERT = report({
  t: { type: "transport", remoteCertificateId: "c" },
  c: { type: "certificate", fingerprint: "AB:CD:EF", fingerprintAlgorithm: "sha-256" },
});

class FakeChannel {
  readyState = "connecting";
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  sent: string[] = [];
  closed = false;
  constructor(readonly label = "") {}
  send(text: string): void {
    this.sent.push(text);
  }
  close(): void {
    this.closed = true;
  }
  open(): void {
    this.readyState = "open";
    this.onopen?.();
  }
}

class FakeLine {
  direction = "recvonly";
  replaced: Array<unknown> = [];
  failReplace = false;
  receiver = { track: { kind: "audio" } };
  sender = {
    replaceTrack: (track: unknown): Promise<void> => {
      if (this.failReplace) return Promise.reject(new Error("no"));
      this.replaced.push(track);
      return Promise.resolve();
    },
  };
}

class FakePc {
  static last: FakePc | null = null;
  config: RTCConfiguration;
  connectionState = "new";
  localDescription: { type: string; sdp: string } | null = null;
  remote: { type: string; sdp: string } | null = null;
  added: RTCIceCandidateInit[] = [];
  transceivers: Array<{ direction?: string }> = [];
  /** The audio transceivers a remote offer created (answerer side). */
  remoteLines: FakeLine[] = [new FakeLine(), new FakeLine()];
  channel: FakeChannel | null = null;
  closed = false;
  statsQueue: RTCStatsReport[] = [];
  statsCalls = 0;
  onicecandidate: ((ev: { candidate: unknown }) => void) | null = null;
  ondatachannel: ((ev: { channel: unknown }) => void) | null = null;
  ontrack: ((ev: unknown) => void) | null = null;
  onconnectionstatechange: (() => void) | null = null;
  failRemote = false;

  constructor(config: RTCConfiguration) {
    this.config = config;
    FakePc.last = this;
  }
  addTransceiver(_track: unknown, init: { direction?: string }): void {
    this.transceivers.push(init);
  }
  getTransceivers(): FakeLine[] {
    return this.remoteLines;
  }
  createDataChannel(label: string): FakeChannel {
    this.channel = new FakeChannel(label);
    return this.channel;
  }
  createOffer(): Promise<{ type: string; sdp: string }> {
    return Promise.resolve({ type: "offer", sdp: "offer-sdp" });
  }
  createAnswer(): Promise<{ type: string; sdp: string }> {
    return Promise.resolve({ type: "answer", sdp: "answer-sdp" });
  }
  setLocalDescription(d: { type: string; sdp: string }): Promise<void> {
    this.localDescription = d;
    return Promise.resolve();
  }
  setRemoteDescription(d: { type: string; sdp: string }): Promise<void> {
    if (this.failRemote) return Promise.reject(new Error("bad sdp"));
    this.remote = d;
    return Promise.resolve();
  }
  addIceCandidate(c: RTCIceCandidateInit): Promise<void> {
    this.added.push(c);
    return Promise.resolve();
  }
  getStats(): Promise<RTCStatsReport> {
    this.statsCalls++;
    return Promise.resolve(this.statsQueue.shift() ?? report({}));
  }
  close(): void {
    this.closed = true;
  }
}

class Recorder implements AttemptListener {
  log: string[] = [];
  onLocalDescription(type: string, sdp: string): void {
    this.log.push(`desc:${type}:${sdp}`);
  }
  onLocalIce(c: IceCandidate): void {
    this.log.push(`ice:${c.candidate}:${c.sdp_mid}:${c.sdp_m_line_index}`);
  }
  onDataOpen(): void {
    this.log.push("open");
  }
  onDataMessage(text: string): void {
    this.log.push(`msg:${text}`);
  }
  onDataClosed(): void {
    this.log.push("dataclosed");
  }
  onConnectionState(state: string): void {
    this.log.push(`state:${state}`);
  }
  onDtlsFingerprint(fp: string): void {
    this.log.push(`fp:${fp}`);
  }
}

const flush = async (): Promise<void> => {
  for (let i = 0; i < 5; i++) await Promise.resolve();
};

function make(offerer: boolean, send: boolean = false) {
  const time = new FakeTime();
  const listener = new Recorder();
  const remoteAudio: string[] = [];
  const attempt = createRtcAttempt(
    {
      PeerConnection: FakePc as unknown as PeerConnectionCtor,
      iceServers: [{ urls: ["stun:relay.test:3478"] }],
      timers: time,
      send: send ? { track: {} as MediaStreamTrack, stream: {} as MediaStream } : null,
      onRemoteAudio: (track) => remoteAudio.push(track.kind),
    },
    offerer,
    listener,
  );
  const pc = FakePc.last as FakePc;
  return { attempt, pc, listener, time, remoteAudio };
}

let attempt_: ReturnType<typeof createRtcAttempt>;

describe("createRtcAttempt", () => {
  it("uses only the given ICE servers", () => {
    const { pc } = make(true);
    expect(pc.config.iceServers).toEqual([{ urls: ["stun:relay.test:3478"] }]);
  });

  it("offerer: creates the channel, the offer and reports the local description", async () => {
    const { attempt, pc, listener } = make(true, true);
    expect(pc.channel?.label).toBe("guardedsleep");
    // The monitoring line first, then the talk line the baby only receives on.
    expect(pc.transceivers).toEqual([
      { direction: "sendonly", streams: expect.anything() },
      { direction: "recvonly" },
    ]);
    attempt.createOffer();
    await flush();
    expect(listener.log).toEqual(["desc:offer:offer-sdp"]);
    pc.channel?.open();
    pc.channel?.open();
    expect(listener.log.filter((l) => l === "open")).toHaveLength(1);
    expect(attempt.sendData("hi")).toBe(true);
    expect(pc.channel?.sent).toEqual(["hi"]);
  });

  it("answerer: waits for the remote description, queues early ICE, then answers", async () => {
    const { attempt, pc, listener } = make(false);
    attempt.createAnswer();
    attempt.addRemoteIce({ candidate: "early", sdp_mid: null, sdp_m_line_index: null });
    await flush();
    expect(listener.log).toEqual([]);
    expect(pc.added).toEqual([]);
    attempt.setRemoteDescription("offer", "remote");
    await flush();
    expect(pc.remote).toEqual({ type: "offer", sdp: "remote" });
    expect(pc.added).toEqual([{ candidate: "early", sdpMid: "0", sdpMLineIndex: 0 }]);
    expect(listener.log).toEqual(["desc:answer:answer-sdp"]);
    attempt.addRemoteIce({ candidate: "late", sdp_mid: "1", sdp_m_line_index: 2 });
    expect(pc.added.at(-1)).toEqual({ candidate: "late", sdpMid: "1", sdpMLineIndex: 2 });
  });

  it("answerer: takes the data channel the baby opens", () => {
    const { pc, listener } = make(false);
    const channel = new FakeChannel();
    pc.ondatachannel?.({ channel });
    channel.open();
    channel.onmessage?.({ data: "hello" });
    channel.onmessage?.({ data: new ArrayBuffer(1) });
    channel.readyState = "closed";
    channel.onclose?.();
    expect(listener.log).toEqual(["open", "msg:hello", "dataclosed"]);
  });

  it("forwards local candidates and ignores the end-of-candidates marker", () => {
    const { pc, listener } = make(true);
    pc.onicecandidate?.({ candidate: { candidate: "c1", sdpMid: "0", sdpMLineIndex: 0 } });
    pc.onicecandidate?.({ candidate: null });
    expect(listener.log).toEqual(["ice:c1:0:0"]);
  });

  it("hands received audio to the player (audio only)", () => {
    const { pc, remoteAudio } = make(false);
    pc.ontrack?.({ track: { kind: "video" }, streams: [] });
    pc.ontrack?.({ track: { kind: "audio" }, streams: [{}] });
    expect(remoteAudio).toEqual(["audio"]);
  });

  it("reports a failed negotiation as a failed connection state", async () => {
    const { attempt, pc, listener } = make(false);
    pc.failRemote = true;
    attempt.setRemoteDescription("offer", "x");
    await flush();
    expect(listener.log).toEqual(["state:failed"]);
  });

  describe("DTLS fingerprint", () => {
    it("reports the remote certificate fingerprint once the connection is up", async () => {
      const { pc, listener } = make(true);
      pc.statsQueue = [WITH_CERT];
      pc.connectionState = "connected";
      pc.onconnectionstatechange?.();
      await flush();
      expect(listener.log).toEqual(["state:connected", "fp:AB:CD:EF"]);
    });

    it("retries while the stats have no certificate and reports exactly once", async () => {
      const { pc, listener, time } = make(true);
      pc.statsQueue = [report({}), report({ t: { type: "transport" } }), WITH_CERT, WITH_CERT];
      pc.connectionState = "connected";
      pc.onconnectionstatechange?.();
      await flush();
      expect(listener.log.filter((l) => l.startsWith("fp:"))).toHaveLength(0);
      for (let i = 0; i < 3; i++) {
        time.advance(FINGERPRINT_RETRY_MS);
        await flush();
      }
      pc.onconnectionstatechange?.(); // a second "connected" event must not report again
      await flush();
      expect(listener.log.filter((l) => l.startsWith("fp:"))).toEqual(["fp:AB:CD:EF"]);
    });

    it("gives up after the bounded number of tries", async () => {
      const { pc, time } = make(true);
      pc.connectionState = "connected";
      pc.onconnectionstatechange?.();
      for (let i = 0; i < 20; i++) {
        await flush();
        time.advance(FINGERPRINT_RETRY_MS);
      }
      expect(pc.statsCalls).toBe(10);
      expect(time.pending()).toBe(0);
    });

    it("accepts SHA-256 only", () => {
      const md5 = report({
        t: { type: "transport", remoteCertificateId: "c" },
        c: { type: "certificate", fingerprint: "AB", fingerprintAlgorithm: "md5" },
      });
      expect(remoteFingerprint(md5)).toBeNull();
      expect(remoteFingerprint(WITH_CERT)).toBe("AB:CD:EF");
    });
  });

  it("parent: reads inbound audio counters only once audio arrives", async () => {
    const { attempt, pc } = make(false);
    expect(await attempt.inboundAudioStats()).toBeNull();
    pc.ontrack?.({ track: { kind: "audio" }, streams: [] });
    pc.statsQueue = [
      report({ r: { type: "inbound-rtp", kind: "audio", packetsReceived: 100, packetsLost: 3 } }),
    ];
    expect(await attempt.inboundAudioStats()).toEqual({ packetsReceived: 100, packetsLost: 3 });
  });

  describe("talk line", () => {
    it("parent: answers the second audio line sendonly, before the answer is created", async () => {
      const { attempt, pc } = make(false);
      attempt.setRemoteDescription("offer", "remote");
      attempt.createAnswer();
      await flush();
      expect(pc.remoteLines.map((l) => l.direction)).toEqual(["recvonly", "sendonly"]);
    });

    it("parent: leaves the talk line recvonly where talk-back is not supported (iOS)", async () => {
      const time = new FakeTime();
      attempt_ = createRtcAttempt(
        {
          PeerConnection: FakePc as unknown as PeerConnectionCtor,
          iceServers: [],
          timers: time,
          send: null,
          onRemoteAudio: () => undefined,
          talkSend: false,
        },
        false,
        new Recorder(),
      );
      attempt_.setRemoteDescription("offer", "remote");
      await flush();
      expect((FakePc.last as FakePc).remoteLines.map((l) => l.direction)).toEqual([
        "recvonly",
        "recvonly",
      ]);
    });

    it("parent: puts a track on the talk sender only on request and takes it off again", async () => {
      const { attempt, pc } = make(false);
      attempt.setRemoteDescription("offer", "remote");
      await flush();
      const track = {} as MediaStreamTrack;
      expect(await attempt.setTalkTrack(track)).toBe(true);
      expect(await attempt.setTalkTrack(null)).toBe(true);
      expect(pc.remoteLines[1]?.replaced).toEqual([track, null]);
      expect(pc.remoteLines[0]?.replaced).toEqual([]);
      pc.remoteLines[1]!.failReplace = true;
      expect(await attempt.setTalkTrack(track)).toBe(false);
    });

    it("parent: no talk sender before the offer is applied", async () => {
      const { attempt } = make(false);
      expect(await attempt.setTalkTrack({} as MediaStreamTrack)).toBe(false);
    });

    it("baby: hands the receiver of the talk line over disabled, never as monitoring audio", () => {
      const talk: Array<boolean> = [];
      const time = new FakeTime();
      const a = createRtcAttempt(
        {
          PeerConnection: FakePc as unknown as PeerConnectionCtor,
          iceServers: [],
          timers: time,
          send: { track: {} as MediaStreamTrack, stream: {} as MediaStream },
          onRemoteAudio: () => talk.push(true),
          onTalkAudio: (track) => talk.push(track.enabled),
        },
        true,
        new Recorder(),
      );
      void a;
      const track = { kind: "audio", enabled: true };
      (FakePc.last as FakePc).ontrack?.({ track, streams: [{}] });
      expect(talk).toEqual([false]);
    });
  });

  it("close releases everything and silences late events", async () => {
    const { attempt, pc, listener, time } = make(true);
    pc.connectionState = "connected";
    pc.onconnectionstatechange?.();
    await flush();
    attempt.close();
    expect(pc.closed).toBe(true);
    expect(pc.channel?.closed).toBe(true);
    expect(time.pending()).toBe(0);
    expect(pc.onconnectionstatechange).toBeNull();
    expect(attempt.sendData("x")).toBe(false);
    const before = listener.log.length;
    pc.channel?.onopen?.();
    expect(listener.log).toHaveLength(before);
  });
});
