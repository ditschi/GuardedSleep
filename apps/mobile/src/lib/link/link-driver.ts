import type {
  CoreApi,
  CoreLink,
  DeviceStatus,
  IceCandidate,
  LinkAction,
  LinkConfig,
  LinkEvent,
  LinkInput,
  LinkStatus,
} from "@guardedsleep/core-wasm";
import { createAttemptPeer, type AttemptFactory, type AttemptPeer } from "./attempt-peer.js";
import { createMonotonicClock, workerTicker, type TickerFactory } from "./clock.js";

/** The core wants a tick at least every 500 ms while monitoring (link.rs module docs). */
export const TICK_INTERVAL_MS = 250;

/** The subset of `WebSocket` the driver uses; tests substitute a fake. */
export interface SocketLike {
  onopen: ((ev: unknown) => void) | null;
  onmessage: ((ev: { data: unknown }) => void) | null;
  onclose: ((ev: unknown) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  send(data: string): void;
  close(): void;
}

/** Baby-side audio hooks the driver needs; implemented by `BabyAudio`. */
export interface BabyAudioPort {
  /** Clone of the microphone track, disabled until the core enables the mic. */
  readonly outgoingTrack: MediaStreamTrack | null;
  setMicEnabled(enabled: boolean): void;
  /** Mixes a tone into the outgoing stream via `sender.replaceTrack`; never plays it locally. */
  playProbeTone(durationMs: number, sender: RTCRtpSender | null): void;
}

export type LinkDriverEnv = {
  wallNow: () => number;
  entropy: () => number;
  createSocket: (url: string) => SocketLike;
  createAttempt: AttemptFactory;
  startTicker: TickerFactory;
  /** Subscribe to "network is back"; returns the unsubscribe function. */
  onNetworkOnline: (cb: () => void) => () => void;
};

export function defaultEnv(): LinkDriverEnv {
  return {
    wallNow: () => Date.now(),
    entropy: () => {
      const buf = new Uint32Array(2);
      crypto.getRandomValues(buf);
      return buf[0]! * 0x1_0000_0000 + buf[1]!;
    },
    createSocket: (url) => new WebSocket(url) as unknown as SocketLike,
    createAttempt: (init) => createAttemptPeer(init),
    startTicker: workerTicker,
    onNetworkOnline: (cb) => {
      if (typeof window === "undefined") return () => {};
      window.addEventListener("online", cb);
      return () => window.removeEventListener("online", cb);
    },
  };
}

export type LinkDriverOptions = {
  core: CoreApi;
  config: LinkConfig;
  /** Tried in order; the next one is used after a socket failed before it ever opened. */
  signalingUrls: string[];
  env?: Partial<LinkDriverEnv>;
  /** Baby only. */
  babyAudio?: BabyAudioPort;
  onEvent?: (event: LinkEvent) => void;
  onRemoteStream?: (stream: MediaStream) => void;
  onError?: (error: unknown) => void;
  /**
   * The browser never exposes the DTLS certificate in its stats (e.g. Firefox), so the mandatory
   * fingerprint check can never pass. Fires once the retries ran out; may fire again per attempt.
   */
  onUnverifiable?: () => void;
  /** A gap between two ticks this large means the page or machine was suspended. */
  onTimeJump?: (gapMs: number) => void;
};

/** Ticks arrive every 250 ms; a gap beyond this is a suspend/resume or a frozen tab. */
export const TIME_JUMP_THRESHOLD_MS = 5_000;

/** Certificate stats may trail `connectionState === "connected"` by a moment. */
const FINGERPRINT_RETRY_MS = 200;
const FINGERPRINT_RETRIES = 25;

type PeerSlot = {
  peer: AttemptPeer;
  fingerprintSent: boolean;
  /** Local ICE is held back until the local description reached the core. */
  descriptionFed: boolean;
  pendingIce: IceCandidate[];
  /** Serializes offer/answer/remote-description steps of this attempt. */
  chain: Promise<void>;
};

/**
 * Hosts one `WasmLink` for one monitoring run (ADR-017). It owns no policy: it feeds platform
 * facts to the core and executes the actions it returns. All retry, backoff, escalation and
 * stop-notice logic lives in the core.
 */
export class LinkDriver {
  private readonly env: LinkDriverEnv;
  private readonly monotonic: () => number;
  private link: CoreLink | null = null;
  private stopTicker: (() => void) | null = null;
  private stopOnline: (() => void) | null = null;
  private queue: LinkInput[] = [];
  private pumping = false;

  private socket: SocketLike | null = null;
  private socketEverOpened = false;
  private urlIndex = 0;

  private readonly slots = new Map<number, PeerSlot>();
  private currentAttempt: number | null = null;
  private lastStatus: LinkStatus | null = null;
  private micEnabled = false;

  constructor(private readonly opts: LinkDriverOptions) {
    this.env = { ...defaultEnv(), ...opts.env };
    this.monotonic = createMonotonicClock(this.env.wallNow);
  }

  get roomId(): string {
    return this.link?.roomId() ?? "";
  }

  get status(): LinkStatus | null {
    return this.lastStatus;
  }

  get isRunning(): boolean {
    return this.link !== null;
  }

  /** Monotonic core clock: use it for every `until_ms` and elapsed-time calculation. */
  now(): number {
    return this.monotonic();
  }

  /** Dev / E2E only. */
  currentPeer(): AttemptPeer | null {
    return this.currentAttempt == null ? null : (this.slots.get(this.currentAttempt)?.peer ?? null);
  }

  micIsEnabled(): boolean {
    return this.micEnabled;
  }

  start(): void {
    if (this.link) return;
    this.link = this.opts.core.createLink(this.opts.config);
    this.stopTicker = this.env.startTicker(() => this.onTick(), TICK_INTERVAL_MS);
    this.stopOnline = this.env.onNetworkOnline(() => this.feed({ type: "network_available" }));
    this.feed({ type: "start", entropy: this.env.entropy() });
  }

  /** Sends the stop notice (core), then releases every resource. Safe to call twice. */
  stop(): void {
    if (!this.link) return;
    this.feed({ type: "stop" });
    this.dispose();
  }

  private lastTickAt: number | null = null;

  private onTick(): void {
    const now = this.monotonic();
    if (this.lastTickAt !== null && now - this.lastTickAt > TIME_JUMP_THRESHOLD_MS) {
      this.opts.onTimeJump?.(now - this.lastTickAt);
    }
    this.lastTickAt = now;
    this.feed({ type: "tick" });
  }

  feed(input: LinkInput): void {
    if (!this.link) return;
    this.queue.push(input);
    if (this.pumping) return;
    this.pumping = true;
    try {
      while (this.queue.length > 0 && this.link) {
        const next = this.queue.shift()!;
        let actions: LinkAction[];
        try {
          actions = this.link.handle(this.monotonic(), next);
        } catch (e) {
          this.opts.onError?.(e);
          continue;
        }
        for (const action of actions) this.exec(action);
      }
    } finally {
      this.pumping = false;
    }
  }

  acknowledgeAlarm(): void {
    this.feed({ type: "acknowledge_alarm" });
  }

  networkAvailable(): void {
    this.feed({ type: "network_available" });
  }

  setListen(enabled: boolean): void {
    this.feed({ type: "set_listen", enabled });
  }

  requestProbe(): void {
    this.feed({ type: "request_probe" });
  }

  /** `untilMs` is on the driver clock (`now()`); null clears the snooze. */
  setSnooze(untilMs: number | null): void {
    this.feed({ type: "set_snooze", until_ms: untilMs });
  }

  requestQuiet(): void {
    this.feed({ type: "request_quiet" });
  }

  audioLevel(rms: number): void {
    this.feed({ type: "audio_level", rms });
  }

  deviceStatus(status: DeviceStatus): void {
    this.feed({ type: "device_status", status });
  }

  private dispose(): void {
    this.stopTicker?.();
    this.stopOnline?.();
    this.stopTicker = null;
    this.stopOnline = null;
    this.closeSocket();
    for (const slot of this.slots.values()) slot.peer.close();
    this.slots.clear();
    this.currentAttempt = null;
    if (this.micEnabled) this.opts.babyAudio?.setMicEnabled(false);
    this.micEnabled = false;
    const link = this.link;
    this.link = null;
    this.queue = [];
    link?.free();
  }

  private exec(action: LinkAction): void {
    switch (action.type) {
      case "open_mailbox":
        this.openSocket();
        break;
      case "close_mailbox":
        this.closeSocket();
        break;
      case "mailbox_send":
        try {
          this.socket?.send(action.frame);
        } catch (e) {
          this.opts.onError?.(e);
        }
        break;
      case "create_attempt":
        this.createAttempt(action.attempt, action.offerer);
        break;
      case "close_attempt":
        this.closeAttempt(action.attempt);
        break;
      case "create_offer":
        this.runStep(action.attempt, (peer) => peer.createOffer());
        break;
      case "create_answer":
        this.runStep(action.attempt, (peer) => peer.createAnswer());
        break;
      case "set_remote_description":
        this.runStep(action.attempt, async (peer) => {
          await peer.setRemoteDescription(action.sdp_type, action.sdp);
          return null;
        });
        break;
      case "add_ice_candidate":
        this.slots.get(action.attempt)?.peer.addIceCandidate(action.candidate);
        break;
      case "send_data":
        this.slots.get(action.attempt)?.peer.send(action.text);
        break;
      case "set_mic_enabled":
        this.micEnabled = action.enabled;
        this.opts.babyAudio?.setMicEnabled(action.enabled);
        break;
      case "apply_quiet":
        // A browser cannot change ringer or Do Not Disturb: answer honestly with an unchanged state.
        this.feed({ type: "quiet_applied", state: { dnd_access: false, quiet_active: false } });
        break;
      case "send_probe_tone":
        this.opts.babyAudio?.playProbeTone(
          action.duration_ms,
          this.currentPeer()?.audioSender() ?? null,
        );
        break;
      case "emit":
        if (action.event.type === "status") this.lastStatus = action.event.status;
        this.opts.onEvent?.(action.event);
        break;
    }
  }

  private openSocket(): void {
    this.closeSocket();
    const urls = this.opts.signalingUrls;
    const url = urls[this.urlIndex % Math.max(1, urls.length)];
    this.socketEverOpened = false;
    let socket: SocketLike;
    try {
      if (!url) throw new Error("no signaling URL");
      socket = this.env.createSocket(url);
    } catch {
      this.urlIndex += 1;
      // Reported asynchronously: the core must not see a re-entrant `mailbox_closed`.
      queueMicrotask(() => this.feed({ type: "mailbox_closed", id: 0, reason: "closed" }));
      return;
    }
    this.socket = socket;
    const current = (): boolean => this.socket === socket;
    socket.onopen = () => {
      if (!current()) return;
      this.socketEverOpened = true;
      this.feed({ type: "mailbox_opened", id: 0 });
    };
    socket.onmessage = (ev) => {
      if (!current() || typeof ev.data !== "string") return;
      this.feed({ type: "mailbox_frame", id: 0, bytes: ev.data });
    };
    const lost = (): void => {
      if (!current()) return;
      this.socket = null;
      if (!this.socketEverOpened && urls.length > 1) this.urlIndex += 1;
      this.feed({ type: "mailbox_closed", id: 0, reason: "closed" });
    };
    socket.onclose = lost;
    socket.onerror = lost;
  }

  private closeSocket(): void {
    const s = this.socket;
    this.socket = null;
    if (!s) return;
    s.onopen = s.onmessage = s.onclose = s.onerror = null;
    try {
      s.close();
    } catch {
      /* already closed */
    }
  }

  private createAttempt(handle: number, offerer: boolean): void {
    const slot: PeerSlot = {
      peer: undefined as unknown as AttemptPeer,
      fingerprintSent: false,
      descriptionFed: false,
      pendingIce: [],
      chain: Promise.resolve(),
    };
    const live = (): boolean => this.slots.get(handle) === slot;
    slot.peer = this.env.createAttempt({
      handle,
      offerer,
      role: this.opts.config.role,
      outgoingTrack: this.opts.babyAudio?.outgoingTrack ?? null,
      callbacks: {
        onLocalIce: (candidate) => {
          if (!live()) return;
          if (slot.descriptionFed)
            this.feed({ type: "attempt_local_ice", attempt: handle, candidate });
          else slot.pendingIce.push(candidate);
        },
        onDataOpen: () => live() && this.feed({ type: "attempt_data_open", attempt: handle }),
        onDataMessage: (text) =>
          live() && this.feed({ type: "attempt_data_message", attempt: handle, text }),
        onDataClosed: () => live() && this.feed({ type: "attempt_data_closed", attempt: handle }),
        onPcState: (state) => {
          if (!live()) return;
          this.feed({ type: "attempt_pc_state", attempt: handle, state });
          if (state === "connected") this.reportDtlsFingerprint(handle, slot);
        },
        onRemoteStream: (stream) => live() && this.opts.onRemoteStream?.(stream),
      },
    });
    this.slots.set(handle, slot);
    this.currentAttempt = handle;
  }

  /**
   * Sends the DTLS-authenticated remote certificate fingerprint once per attempt. Without it the
   * core never verifies the peer (it times the attempt out), so this is not optional.
   */
  private reportDtlsFingerprint(handle: number, slot: PeerSlot, tries = 0): void {
    if (slot.fingerprintSent || this.slots.get(handle) !== slot) return;
    slot.peer.remoteDtlsFingerprint().then(
      (fingerprint) => {
        if (slot.fingerprintSent || this.slots.get(handle) !== slot) return;
        if (fingerprint !== null) {
          slot.fingerprintSent = true;
          this.feed({ type: "attempt_dtls_fingerprint", attempt: handle, fingerprint });
        } else if (tries < FINGERPRINT_RETRIES) {
          setTimeout(
            () => this.reportDtlsFingerprint(handle, slot, tries + 1),
            FINGERPRINT_RETRY_MS,
          );
        } else {
          this.opts.onError?.(new Error("remote DTLS fingerprint unavailable"));
          this.opts.onUnverifiable?.();
        }
      },
      (e: unknown) => {
        if (tries < FINGERPRINT_RETRIES && this.slots.get(handle) === slot) {
          setTimeout(
            () => this.reportDtlsFingerprint(handle, slot, tries + 1),
            FINGERPRINT_RETRY_MS,
          );
        } else this.opts.onError?.(e);
      },
    );
  }

  private closeAttempt(handle: number): void {
    const slot = this.slots.get(handle);
    if (!slot) return;
    this.slots.delete(handle);
    if (this.currentAttempt === handle) this.currentAttempt = null;
    slot.peer.close();
  }

  private runStep(
    handle: number,
    step: (peer: AttemptPeer) => Promise<{ type: string; sdp: string } | null>,
  ): void {
    const slot = this.slots.get(handle);
    if (!slot) return;
    slot.chain = slot.chain
      .then(async () => {
        if (this.slots.get(handle) !== slot) return;
        const desc = await step(slot.peer);
        if (this.slots.get(handle) !== slot || !desc) return;
        this.feed({
          type: "attempt_local_description",
          attempt: handle,
          sdp_type: desc.type,
          sdp: desc.sdp,
        });
        slot.descriptionFed = true;
        const held = slot.pendingIce.splice(0);
        for (const candidate of held) {
          this.feed({ type: "attempt_local_ice", attempt: handle, candidate });
        }
      })
      .catch((e: unknown) => {
        this.opts.onError?.(e);
        if (this.slots.get(handle) === slot) {
          this.feed({ type: "attempt_pc_state", attempt: handle, state: "failed" });
        }
      });
  }
}
