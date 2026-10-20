import { vi } from "vitest";
import type { IceCandidate } from "@guardedsleep/core-wasm";
import type { AttemptCallbacks, AttemptInit, AttemptPeer } from "../attempt-peer.js";
import type { BabyAudioPort, LinkDriverEnv, SocketLike } from "../link-driver.js";

/** In-memory replacement for browser WebSocket / RTCPeerConnection so the real core can be driven. */

type Member = { clientId: string; role: string; socket: FakeSocket };

class FakeRoom {
  members: Member[] = [];
}

/** Minimal signaling server: hello -> welcome + members, signal relay by `to`. */
export class FakeSignalingServer {
  private rooms = new Map<string, FakeRoom>();
  private nextId = 1;
  reachable = true;
  readonly helloCount = { n: 0 };

  connect(url: string): FakeSocket {
    const socket = new FakeSocket(this, url);
    queueMicrotask(() => {
      if (!this.reachable || socket.closedByClient) socket.fail();
      else socket.open();
    });
    return socket;
  }

  /** Called by a socket for every client message. */
  onClientMessage(socket: FakeSocket, raw: string): void {
    const msg = JSON.parse(raw) as Record<string, unknown>;
    if (msg.type === "hello") {
      this.helloCount.n += 1;
      const roomId = String(msg.roomId);
      const room = this.rooms.get(roomId) ?? new FakeRoom();
      this.rooms.set(roomId, room);
      const clientId = `c${this.nextId++}`;
      socket.joined = { roomId, clientId };
      room.members.push({ clientId, role: String(msg.role), socket });
      socket.deliver({ type: "welcome", clientId, roomId });
      this.broadcastMembers(room);
    } else if (msg.type === "ping") {
      socket.deliver({ type: "pong" });
    } else if (msg.type === "signal" && socket.joined) {
      const room = this.rooms.get(socket.joined.roomId);
      if (!room) return;
      const targets = room.members.filter(
        (m) => m.clientId !== socket.joined!.clientId && (!msg.to || m.clientId === msg.to),
      );
      for (const t of targets) {
        t.socket.deliver({ type: "signal", from: socket.joined.clientId, payload: msg.payload });
      }
    }
  }

  onSocketGone(socket: FakeSocket): void {
    if (!socket.joined) return;
    const room = this.rooms.get(socket.joined.roomId);
    if (!room) return;
    room.members = room.members.filter((m) => m.socket !== socket);
    this.broadcastMembers(room);
  }

  /** Simulates a server restart / network cut: every socket drops. */
  dropAll(): void {
    for (const room of this.rooms.values()) {
      for (const m of [...room.members]) m.socket.fail();
    }
  }

  private broadcastMembers(room: FakeRoom): void {
    const peers = room.members.map((m) => ({ clientId: m.clientId, role: m.role }));
    for (const m of room.members) m.socket.deliver({ type: "members", peers });
  }
}

export class FakeSocket implements SocketLike {
  onopen: ((ev: unknown) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onclose: ((ev: unknown) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  joined: { roomId: string; clientId: string } | null = null;
  closedByClient = false;
  private isOpen = false;

  constructor(
    private readonly server: FakeSignalingServer,
    readonly url: string,
  ) {}

  open(): void {
    this.isOpen = true;
    this.onopen?.({});
  }

  fail(): void {
    const wasOpen = this.isOpen;
    this.isOpen = false;
    this.server.onSocketGone(this);
    if (wasOpen) this.onclose?.({});
    else this.onerror?.({});
  }

  deliver(message: object): void {
    if (!this.isOpen) return;
    const text = JSON.stringify(message);
    queueMicrotask(() => this.isOpen && this.onmessage?.({ data: text }));
  }

  send(data: string): void {
    if (!this.isOpen) return;
    this.server.onClientMessage(this, data);
  }

  close(): void {
    this.closedByClient = true;
    if (this.isOpen) {
      this.isOpen = false;
      this.server.onSocketGone(this);
    }
  }
}

let fpCounter = 0;
function fingerprint(): string {
  fpCounter += 1;
  const bytes = Array.from({ length: 32 }, (_, i) =>
    ((fpCounter * 31 + i * 7) & 0xff).toString(16).padStart(2, "0"),
  );
  return bytes.join(":").toUpperCase();
}

function normalizedFp(sdp: string): string | null {
  const m = /a=fingerprint:sha-256 ([0-9A-Fa-f:]+)/.exec(sdp);
  return m ? m[1]!.replace(/:/g, "").toLowerCase() : null;
}

/** Wires fake peers together by DTLS fingerprint, like a network that always works. */
export class FakeNetwork {
  readonly peers: FakePeer[] = [];
  /** Opening is held while false (simulates an unreachable peer). */
  up = true;

  register(peer: FakePeer): void {
    this.peers.push(peer);
  }

  /** Called whenever a peer got a remote description or the network came back. */
  tryConnect(a: FakePeer): void {
    if (!this.up || a.closed || a.connectedTo) return;
    const b = this.peers.find(
      (p) => !p.closed && !p.connectedTo && p !== a && p.normalizedFp === a.remoteFp,
    );
    if (!b || b.remoteFp !== a.normalizedFp) return;
    a.connectedTo = b;
    b.connectedTo = a;
    queueMicrotask(() => {
      for (const p of [a, b]) {
        if (p.closed) continue;
        p.dcOpen = true;
        p.callbacks.onPcState("connected");
        p.callbacks.onDataOpen();
      }
    });
  }

  /** The DataChannel of both ends closes (a dropped connection). */
  dropChannel(a: FakePeer): void {
    const b = a.connectedTo;
    for (const p of [a, b]) {
      if (!p || p.closed) continue;
      p.dcOpen = false;
      p.callbacks.onDataClosed();
      p.callbacks.onPcState("failed");
    }
  }
}

export class FakePeer implements AttemptPeer {
  readonly fp = fingerprint();
  readonly normalizedFp = this.fp.replace(/:/g, "").toLowerCase();
  remoteFp: string | null = null;
  connectedTo: FakePeer | null = null;
  dcOpen = false;
  closed = false;
  readonly receivedIce: IceCandidate[] = [];
  readonly sent: string[] = [];
  readonly pc = {} as RTCPeerConnection;
  remoteDescriptionSetAt: number | null = null;
  private step = 0;

  constructor(
    private readonly net: FakeNetwork,
    readonly init: AttemptInit,
  ) {
    this.callbacks = init.callbacks;
    net.register(this);
  }

  readonly callbacks: AttemptCallbacks;

  private sdp(direction: string): string {
    return `v=0\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111\r\na=${direction}\r\na=fingerprint:sha-256 ${this.fp}\r\nm=application 9 UDP/DTLS/SCTP webrtc-datachannel\r\n`;
  }

  async createOffer(): Promise<{ type: string; sdp: string }> {
    await Promise.resolve();
    queueMicrotask(() =>
      this.callbacks.onLocalIce({
        candidate: "candidate:1 1 udp 1 127.0.0.1 9 typ host",
        sdp_mid: "0",
        sdp_m_line_index: 0,
      }),
    );
    return { type: "offer", sdp: this.sdp("sendonly") };
  }

  async createAnswer(): Promise<{ type: string; sdp: string }> {
    await Promise.resolve();
    return { type: "answer", sdp: this.sdp("recvonly") };
  }

  async setRemoteDescription(_type: string, sdp: string): Promise<void> {
    await Promise.resolve();
    this.remoteFp = normalizedFp(sdp);
    this.remoteDescriptionSetAt = ++this.step;
    this.net.tryConnect(this);
    if (this.connectedTo === null) {
      // The other side may only learn our fingerprint later.
      for (const p of this.net.peers) if (p !== this) this.net.tryConnect(p);
    }
  }

  addIceCandidate(candidate: IceCandidate): void {
    this.receivedIce.push(candidate);
  }

  send(text: string): boolean {
    if (!this.dcOpen || !this.connectedTo) return false;
    this.sent.push(text);
    const to = this.connectedTo;
    queueMicrotask(() => {
      if (to.dcOpen && !to.closed) to.callbacks.onDataMessage(text);
    });
    return true;
  }

  audioSender(): RTCRtpSender | null {
    return null;
  }

  /** Real browsers report the peer's certificate; the fake knows who it is wired to. */
  async remoteDtlsFingerprint(): Promise<string | null> {
    return this.connectedTo?.fp ?? null;
  }

  dataChannel(): RTCDataChannel | null {
    return null;
  }

  close(): void {
    if (this.closed) return;
    this.closed = true;
    const other = this.connectedTo;
    this.connectedTo = null;
    if (other && !other.closed && other.dcOpen) {
      queueMicrotask(() => {
        other.dcOpen = false;
        other.connectedTo = null;
        other.callbacks.onDataClosed();
      });
    }
  }
}

export type TestClock = {
  now: number;
  /** Wall clock that tests can also jump backwards. */
  wallNow: () => number;
};

export type Harness = {
  server: FakeSignalingServer;
  net: FakeNetwork;
  clock: TestClock;
  /** Env for one side; each side gets its own ticker. */
  env: () => { env: Partial<LinkDriverEnv>; tick: () => void };
  /** Advances simulated time in 250 ms steps, ticking every registered side. */
  advance: (ms: number) => Promise<void>;
  flush: () => Promise<void>;
};

/** Settles microtasks and zero-delay timers; requires `vi.useFakeTimers()`. */
export async function flush(): Promise<void> {
  for (let i = 0; i < 6; i++) await vi.advanceTimersByTimeAsync(0);
}

export function createHarness(startAt = 1_000_000): Harness {
  const server = new FakeSignalingServer();
  const net = new FakeNetwork();
  const clock: TestClock = { now: startAt, wallNow: () => clock.now };
  const tickers: (() => void)[] = [];
  let entropy = 1;
  const env = (): { env: Partial<LinkDriverEnv>; tick: () => void } => {
    let handler: (() => void) | null = null;
    const e: Partial<LinkDriverEnv> = {
      wallNow: () => clock.now,
      entropy: () => entropy++ * 7919,
      createSocket: (url) => server.connect(url),
      createAttempt: (init) => new FakePeer(net, init),
      startTicker: (onTick) => {
        handler = onTick;
        tickers.push(onTick);
        return () => {
          handler = null;
          const i = tickers.indexOf(onTick);
          if (i >= 0) tickers.splice(i, 1);
        };
      },
      onNetworkOnline: () => () => {},
    };
    return { env: e, tick: () => handler?.() };
  };
  return {
    server,
    net,
    clock,
    env,
    flush,
    async advance(ms) {
      for (let elapsed = 0; elapsed < ms; elapsed += 250) {
        clock.now += 250;
        for (const t of [...tickers]) t();
        await vi.advanceTimersByTimeAsync(250);
        await flush();
      }
    },
  };
}

export class FakeBabyAudio implements BabyAudioPort {
  outgoingTrack = null;
  enabledLog: boolean[] = [];
  tones: number[] = [];
  setMicEnabled(enabled: boolean): void {
    this.enabledLog.push(enabled);
  }
  playProbeTone(durationMs: number): void {
    this.tones.push(durationMs);
  }
}
