/*
 * Fake environment for the platform tests: a manual clock and timers, a fake WebSocket, a fake
 * RTCPeerConnection and a recording media engine. Test-only; production code never imports it
 * (the bundle only contains what `main.ts` reaches).
 */
import type { IceCandidate, IceServer } from "@guardedsleep/core-wasm";
import type { Cancel, Clock, Timers } from "./clock";
import type { AttemptListener, AttemptPort, InboundAudioStats, MediaEngine } from "./media-engine";
import type { WebSocketLike } from "./signaling";

/** Manual time: `advance` runs due timers in order. The clock reads the same time. */
export class FakeTime implements Clock, Timers {
  now = 0;
  wall = 1_700_000_000_000;
  private seq = 0;
  private readonly tasks = new Map<number, { at: number; every: number | null; fn: () => void }>();

  elapsedMs(): number {
    return this.now;
  }

  wallMs(): number {
    return this.wall + this.now;
  }

  every(fn: () => void, ms: number): Cancel {
    const id = ++this.seq;
    this.tasks.set(id, { at: this.now + ms, every: ms, fn });
    return () => this.tasks.delete(id);
  }

  after(fn: () => void, ms: number): Cancel {
    const id = ++this.seq;
    this.tasks.set(id, { at: this.now + ms, every: null, fn });
    return () => this.tasks.delete(id);
  }

  pending(): number {
    return this.tasks.size;
  }

  advance(ms: number): void {
    const target = this.now + ms;
    for (;;) {
      let nextId: number | null = null;
      let nextAt = Infinity;
      for (const [id, t] of this.tasks) {
        if (t.at <= target && t.at < nextAt) {
          nextAt = t.at;
          nextId = id;
        }
      }
      if (nextId === null) break;
      const task = this.tasks.get(nextId);
      if (!task) break;
      this.now = Math.max(this.now, task.at);
      if (task.every === null) this.tasks.delete(nextId);
      else task.at += task.every;
      task.fn();
    }
    this.now = target;
  }
}

export class FakeWebSocket implements WebSocketLike {
  static instances: FakeWebSocket[] = [];
  readyState = 0;
  onopen: ((ev: unknown) => void) | null = null;
  onmessage: ((ev: { data: unknown }) => void) | null = null;
  onerror: ((ev: unknown) => void) | null = null;
  onclose: ((ev: unknown) => void) | null = null;
  readonly sent: string[] = [];
  closed = false;

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this);
  }

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.closed = true;
    this.readyState = 3;
  }

  // Server side of the fake.
  open(): void {
    this.readyState = 1;
    this.onopen?.({});
  }

  receive(text: string): void {
    this.onmessage?.({ data: text });
  }

  drop(): void {
    this.readyState = 3;
    this.onclose?.({});
  }
}

/** Records what the platform asked of one attempt and lets a test play the browser's part. */
export class FakeAttempt implements AttemptPort {
  readonly calls: string[] = [];
  readonly sent: string[] = [];
  closed = false;
  stats: InboundAudioStats | null = null;

  constructor(
    readonly offerer: boolean,
    readonly listener: AttemptListener,
  ) {}

  createOffer(): void {
    this.calls.push("createOffer");
  }
  setRemoteDescription(type: string, sdp: string): void {
    this.calls.push(`setRemoteDescription:${type}:${sdp}`);
  }
  createAnswer(): void {
    this.calls.push("createAnswer");
  }
  addRemoteIce(candidate: IceCandidate): void {
    this.calls.push(`addRemoteIce:${candidate.candidate}`);
  }
  sendData(text: string): boolean {
    this.sent.push(text);
    return true;
  }
  inboundAudioStats(): Promise<InboundAudioStats | null> {
    return Promise.resolve(this.stats);
  }
  close(): void {
    this.closed = true;
  }
}

export class FakeMedia implements MediaEngine {
  readonly attempts: FakeAttempt[] = [];
  readonly calls: string[] = [];
  micRms: number | null = null;
  remoteRms: number | null = null;
  throwOnCreate = false;
  released = false;

  /** The ICE list each attempt was created with, as the core named it. */
  readonly iceServers: IceServer[][] = [];

  createAttempt(offerer: boolean, listener: AttemptListener, iceServers: IceServer[]): AttemptPort {
    this.iceServers.push(iceServers);
    if (this.throwOnCreate) throw new Error("no peer connection");
    const attempt = new FakeAttempt(offerer, listener);
    this.attempts.push(attempt);
    return attempt;
  }
  setMicEnabled(enabled: boolean): void {
    this.calls.push(`mic:${enabled}`);
  }
  startProbeTone(durationMs: number): void {
    this.calls.push(`probe:${durationMs}`);
  }
  setPlaybackMuted(muted: boolean): void {
    this.calls.push(`muted:${muted}`);
  }
  setPlaybackGainPct(pct: number): void {
    this.calls.push(`gain:${pct}`);
  }
  drainMicRms(): number | null {
    const v = this.micRms;
    this.micRms = null;
    return v;
  }
  drainRemoteRms(): number | null {
    const v = this.remoteRms;
    this.remoteRms = null;
    return v;
  }
  release(): void {
    this.released = true;
  }
}
