import type {
  CoreApi,
  CoreLink,
  DeviceStatus,
  LinkAction,
  LinkConfig,
  LinkEvent,
  LinkInput,
  LinkStatus,
} from "@guardedsleep/core-wasm";
import type { Cancel, Clock, Timers } from "./clock";
import type { AttemptPort, InboundAudioStats, MediaEngine } from "./media-engine";
import {
  AUDIO_LEVEL_MS,
  DEVICE_STATUS_CHECK_MS,
  DEVICE_STATUS_RESEND_MS,
  FLUSH_MS,
  STATS_MS,
  TICK_MS,
} from "./policy";
import type { SignalingFactory, SignalingSocket } from "./signaling";

/** What the controller wants to hear from one run. Called synchronously from the driver. */
export interface LinkListener {
  onStatus(status: LinkStatus, nowMs: number): void;
  onEvent(event: LinkEvent, nowMs: number): void;
  onTick(nowMs: number): void;
  /** Baby: microphone level for the meter, about 10 Hz. */
  onMicLevel(rms: number, nowMs: number): void;
  /** Parent: level of the received audio, about 10 Hz while audio flows. */
  onRemoteLevel(rms: number, nowMs: number): void;
  /** Parent: inbound audio counters every few seconds. */
  onInboundStats(stats: InboundAudioStats, nowMs: number): void;
  /** The run is fully torn down. */
  onTornDown(): void;
}

/** The longest mailbox frame passed on to the core (`signaling-mailboxes`: frames are capped at 64 KiB). */
const MAX_FRAME_CHARS = 64 * 1024;
/** The wall clock is reported this often (the core needs minute resolution). */
const WALL_CLOCK_EVERY_MS = 60_000;
/** A wall clock this far off the monotonic time since the last report counts as a time change. */
const WALL_CLOCK_JUMP_MS = 2_000;

export interface LinkDriverOptions {
  config: LinkConfig;
  core: Pick<CoreApi, "createLink">;
  signaling: SignalingFactory;
  media: MediaEngine;
  clock: Clock;
  timers: Timers;
  listener: LinkListener;
  /** Random 53-bit seed for the core's jitter; the core has no entropy source of its own. */
  entropy: number;
  /** Baby: current device state; absent on the parent. */
  deviceStatus?: () => DeviceStatus;
  /** Baby: apply the silence rule, return the fresh state. */
  applyQuiet?: () => DeviceStatus;
}

/**
 * Executes one monitoring run of the Rust core (`CoreLink`): it feeds the monotonic clock, the
 * signaling socket, the peer connection and the audio levels in as inputs and executes the actions
 * that come out. It contains no decisions: back-off, escalation, attempt numbering and timeouts
 * are all the core's. Inputs are queued, so an action that causes a callback synchronously never
 * re-enters the core.
 */
export class LinkDriver {
  private readonly o: LinkDriverOptions;
  private link: CoreLink | null = null;
  /** One socket per mailbox id, opened and closed only on the core's actions. */
  private readonly sockets = new Map<number, { gen: number; socket: SignalingSocket }>();
  private socketGen = 0;
  private readonly attempts = new Map<number, AttemptPort>();
  private liveAttempt: number | null = null;
  private readonly cancels = new Set<Cancel>();
  private readonly queue: LinkInput[] = [];
  private draining = false;
  private closing = false;
  private tornDown = false;
  private lastSentStatus: DeviceStatus | null = null;
  private lastSentStatusAt = 0;
  private lastWall: { wallMs: number; elapsedMs: number } | null = null;
  private readonly isBaby: boolean;

  constructor(options: LinkDriverOptions) {
    this.o = options;
    this.isBaby = options.config.role === "baby";
  }

  /** Builds the core link, hands it the start input and starts the platform timers. */
  start(): void {
    if (this.link || this.tornDown) return;
    try {
      this.link = this.o.core.createLink(this.o.config);
      // The core took the seed; do not keep it around (best effort: a JS string cannot be wiped).
      const { pinned_parent: _kept, ...rest } = this.o.config;
      this.o.config = { ...rest, seed: "" };
    } catch {
      // A rejected config is a programming error, never a runtime state; nothing to run.
      this.teardown();
      return;
    }
    this.dispatch({ type: "start", entropy: this.o.entropy });
    this.reportWallClock();
    this.every(() => {
      this.dispatch({ type: "tick" });
      this.reportWallClock();
      this.o.listener.onTick(this.o.clock.elapsedMs());
    }, TICK_MS);
    this.every(() => this.sampleAudio(), AUDIO_LEVEL_MS);
    if (this.isBaby) this.every(() => this.sendDeviceStatus(false), DEVICE_STATUS_CHECK_MS);
    else this.every(() => this.pollStats(), STATS_MS);
  }

  /**
   * The wall clock in Unix seconds, for the `here` message and the clock comparison with the peer only (never liveness): sent at start,
   * every 60 s, and when a tick sees the clock jump (more than 2 s off the elapsed monotonic time
   * since the last report), which covers a changed device time.
   */
  private reportWallClock(): void {
    const wallMs = Date.now();
    const elapsed = this.o.clock.elapsedMs();
    const last = this.lastWall;
    if (last) {
      const sinceMs = elapsed - last.elapsedMs;
      const jumpMs = Math.abs(wallMs - last.wallMs - sinceMs);
      if (sinceMs < WALL_CLOCK_EVERY_MS && jumpMs <= WALL_CLOCK_JUMP_MS) return;
    }
    this.lastWall = { wallMs, elapsedMs: elapsed };
    this.dispatch({ type: "wall_clock", unix_s: Math.floor(wallMs / 1000) });
  }

  /** Announces a deliberate stop to the peer, then tears down after a short flush. */
  stop(): void {
    if (this.closing || this.tornDown) return;
    // Set first: the close_attempt the stop produces must wait for the flush.
    this.closing = true;
    this.dispatch({ type: "stop" });
    this.after(() => this.teardown(), FLUSH_MS + 200);
  }

  /** Immediate teardown without telling the peer (page leave, replaced run). */
  abort(): void {
    this.teardown();
  }

  input(input: LinkInput): void {
    if (!this.closing) this.dispatch(input);
  }

  /** Runs a media call (gain, mute) on the run's engine. */
  media(action: (media: MediaEngine) => void): void {
    if (!this.tornDown) action(this.o.media);
  }

  private dispatch(input: LinkInput): void {
    if (this.tornDown || !this.link) return;
    this.queue.push(input);
    if (this.draining) return;
    this.draining = true;
    try {
      for (let next = this.queue.shift(); next; next = this.queue.shift()) {
        if (this.tornDown || !this.link) break;
        const now = this.o.clock.elapsedMs();
        let actions: LinkAction[];
        try {
          actions = this.link.handle(now, next);
        } catch {
          // Malformed core output or input: drop this step, the next tick carries on.
          continue;
        }
        for (const action of actions) this.execute(action, now);
      }
    } finally {
      this.queue.length = 0;
      this.draining = false;
    }
  }

  private execute(action: LinkAction, now: number): void {
    switch (action.type) {
      case "open_mailbox":
        return this.openMailbox(action.id, action.url);
      case "close_mailbox":
        return this.closeMailbox(action.id);
      case "mailbox_send":
        return this.sockets.get(action.id)?.socket.send(action.frame);
      case "create_attempt":
        return this.createAttempt(action.attempt, action.offerer);
      case "close_attempt":
        return this.closeAttempt(action.attempt);
      case "create_offer":
        return this.attempts.get(action.attempt)?.createOffer();
      case "set_remote_description":
        return this.attempts.get(action.attempt)?.setRemoteDescription(action.sdp_type, action.sdp);
      case "create_answer":
        return this.attempts.get(action.attempt)?.createAnswer();
      case "add_ice_candidate":
        return this.attempts.get(action.attempt)?.addRemoteIce(action.candidate);
      case "send_data":
        this.attempts.get(action.attempt)?.sendData(action.text);
        return;
      case "set_mic_enabled":
        return this.o.media.setMicEnabled(action.enabled);
      case "apply_quiet":
        return this.applyQuietNow();
      case "send_probe_tone":
        return this.o.media.startProbeTone(action.duration_ms);
      case "emit":
        if (action.event.type === "status") this.o.listener.onStatus(action.event.status, now);
        else this.o.listener.onEvent(action.event, now);
        return;
    }
  }

  /**
   * The core's `url` is empty for the relay mailbox: its URL list still lives here. A mailbox
   * that names its address gets only that address tried. A failed open comes back as
   * `mailbox_closed`, never as a retry.
   */
  private openMailbox(id: number, url: string): void {
    this.closeMailbox(id);
    const gen = ++this.socketGen;
    // Callbacks of a replaced socket are dropped; the core only hears about the current one.
    const current = (): boolean => this.sockets.get(id)?.gen === gen;
    const socket = this.o.signaling(url, {
      onOpen: () => {
        if (current()) this.dispatch({ type: "mailbox_opened", id });
      },
      onMessage: (bytes) => {
        // A frame over the cap is dropped here (a relay is untrusted); the core caps too.
        if (current() && bytes.length <= MAX_FRAME_CHARS) {
          this.dispatch({ type: "mailbox_frame", id, bytes });
        }
      },
      onClose: () => {
        if (current()) this.dispatch({ type: "mailbox_closed", id, reason: "closed" });
      },
    });
    this.sockets.set(id, { gen, socket });
  }

  private closeMailbox(id: number): void {
    this.sockets.get(id)?.socket.close();
    this.sockets.delete(id);
  }

  private createAttempt(handle: number, offerer: boolean): void {
    try {
      const port = this.o.media.createAttempt(offerer, {
        onLocalDescription: (sdp_type, sdp) =>
          this.whenKnown(handle, {
            type: "attempt_local_description",
            attempt: handle,
            sdp_type,
            sdp,
          }),
        onLocalIce: (candidate) =>
          this.whenKnown(handle, { type: "attempt_local_ice", attempt: handle, candidate }),
        onDataOpen: () => this.whenKnown(handle, { type: "attempt_data_open", attempt: handle }),
        onDataMessage: (text) =>
          this.whenKnown(handle, { type: "attempt_data_message", attempt: handle, text }),
        onDataClosed: () =>
          this.whenKnown(handle, { type: "attempt_data_closed", attempt: handle }),
        onDtlsFingerprint: (fingerprint) =>
          this.whenKnown(handle, {
            type: "attempt_dtls_fingerprint",
            attempt: handle,
            fingerprint,
          }),
        onConnectionState: (state) =>
          this.whenKnown(handle, { type: "attempt_pc_state", attempt: handle, state }),
      });
      this.attempts.set(handle, port);
      this.liveAttempt = handle;
    } catch {
      // The core sees a failed attempt and schedules the next one.
      this.after(
        () => this.dispatch({ type: "attempt_pc_state", attempt: handle, state: "failed" }),
        0,
      );
    }
  }

  private whenKnown(handle: number, input: LinkInput): void {
    if (this.attempts.has(handle)) this.dispatch(input);
  }

  private closeAttempt(handle: number): void {
    const port = this.attempts.get(handle);
    if (!port) return;
    if (this.liveAttempt === handle) this.liveAttempt = null;
    if (this.closing) {
      // A stop notice was just written to the DataChannel; let it leave before the close.
      this.after(() => {
        this.attempts.delete(handle);
        port.close();
      }, FLUSH_MS);
    } else {
      this.attempts.delete(handle);
      port.close();
    }
  }

  private applyQuietNow(): void {
    const apply = this.o.applyQuiet;
    if (!apply) return;
    const state = apply();
    this.lastSentStatus = state;
    this.lastSentStatusAt = this.o.clock.elapsedMs();
    this.dispatch({ type: "quiet_applied", state });
  }

  private sampleAudio(): void {
    const now = this.o.clock.elapsedMs();
    if (this.isBaby) {
      const rms = this.o.media.drainMicRms();
      if (rms !== null) {
        this.o.listener.onMicLevel(rms, now);
        this.dispatch({ type: "audio_level", rms });
      }
    } else {
      const rms = this.o.media.drainRemoteRms();
      if (rms !== null) this.o.listener.onRemoteLevel(rms, now);
    }
  }

  private sendDeviceStatus(force: boolean): void {
    const read = this.o.deviceStatus;
    if (!read) return;
    const now = this.o.clock.elapsedMs();
    const status = read();
    const changed = !sameStatus(status, this.lastSentStatus);
    if (force || changed || now - this.lastSentStatusAt >= DEVICE_STATUS_RESEND_MS) {
      this.lastSentStatus = status;
      this.lastSentStatusAt = now;
      this.dispatch({ type: "device_status", status });
    }
  }

  private pollStats(): void {
    const handle = this.liveAttempt;
    const port = handle === null ? undefined : this.attempts.get(handle);
    if (!port) return;
    port.inboundAudioStats().then(
      (stats) => {
        if (stats && !this.tornDown)
          this.o.listener.onInboundStats(stats, this.o.clock.elapsedMs());
      },
      () => undefined,
    );
  }

  private teardown(): void {
    if (this.tornDown) return;
    this.tornDown = true;
    for (const cancel of this.cancels) cancel();
    this.cancels.clear();
    for (const { socket } of this.sockets.values()) socket.close();
    this.sockets.clear();
    for (const port of this.attempts.values()) {
      try {
        port.close();
      } catch {
        // Best effort: the peer connection is going away anyway.
      }
    }
    this.attempts.clear();
    this.o.media.release();
    this.link?.free();
    this.link = null;
    this.o.listener.onTornDown();
  }

  private every(fn: () => void, ms: number): void {
    this.cancels.add(this.o.timers.every(fn, ms));
  }

  private after(fn: () => void, ms: number): void {
    const cancel = this.o.timers.after(() => {
      this.cancels.delete(cancel);
      fn();
    }, ms);
    this.cancels.add(cancel);
  }
}

/** Battery is rounded to 5 % on the wire; a smaller change is not a change worth sending. */
function sameStatus(a: DeviceStatus, b: DeviceStatus | null): boolean {
  if (b === null) return false;
  const round = (s: DeviceStatus): string =>
    JSON.stringify({
      ...s,
      battery_pct: s.battery_pct === undefined ? undefined : Math.floor(s.battery_pct / 5) * 5,
    });
  return round(a) === round(b);
}
