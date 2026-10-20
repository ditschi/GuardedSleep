import { get, writable, type Readable } from "svelte/store";
import type { CoreApi, LinkConfig, LinkEvent } from "@guardedsleep/core-wasm";
import { LinkDriver, type LinkDriverEnv } from "./link-driver.js";
import {
  QUALITY_WINDOW_MS,
  qualityFromWindow,
  readInboundAudioCounters,
  type AudioCounters,
} from "./link-quality.js";
import {
  markBrowserUnsupported,
  reduceParentEvent,
  tickParentState,
  type EventContext,
} from "./monitor-state.js";
import { initialParentState, type ListenMode, type ParentMonitorState } from "./ui-contract.js";

/** "Connected again after m:ss" stays visible this long. */
export const RESTORED_BANNER_MS = 5_000;
/** The baby has this long to confirm a remote silence request. */
export const SILENCE_REQUEST_TIMEOUT_MS = 10_000;
const HOUSEKEEPING_MS = 1_000;

export type ParentMonitorOptions = {
  core: CoreApi;
  config: LinkConfig;
  signalingUrls: string[];
  env?: Partial<LinkDriverEnv>;
  wallNow?: () => number;
  onRemoteStream?: (stream: MediaStream) => void;
};

/**
 * One parent monitoring run: a `LinkDriver` plus the reduction of core events into
 * `ParentMonitorState`. Methods mirror the `AppController` parent actions of the native contract.
 */
export class ParentMonitor {
  private readonly store = writable<ParentMonitorState>(initialParentState());
  readonly state: Readable<ParentMonitorState> = { subscribe: this.store.subscribe };
  readonly remoteStream = writable<MediaStream | null>(null);

  /** Wall time of the last detected suspend/resume gap, with its length; null if none happened. */
  readonly suspendGap = writable<{ atWallMs: number; gapMs: number } | null>(null);

  private readonly driver: LinkDriver;
  private readonly wallNow: () => number;
  private readonly listeners = new Set<(ev: LinkEvent) => void>();
  private housekeeping: ReturnType<typeof setInterval> | null = null;
  private restoredTimer: ReturnType<typeof setTimeout> | null = null;
  private silenceTimer: ReturnType<typeof setTimeout> | null = null;
  private qualityTimer: ReturnType<typeof setInterval> | null = null;
  private qualityBase: AudioCounters | null = null;
  private noiseAlerts = 0;
  private unverifiable = false;

  constructor(private readonly opts: ParentMonitorOptions) {
    this.wallNow = opts.wallNow ?? (() => Date.now());
    this.driver = new LinkDriver({
      core: opts.core,
      config: opts.config,
      signalingUrls: opts.signalingUrls,
      env: opts.env,
      onTimeJump: (gapMs) => this.suspendGap.set({ atWallMs: this.wallNow(), gapMs }),
      onEvent: (ev) => this.onEvent(ev),
      onUnverifiable: () => {
        this.unverifiable = true;
        this.store.update((s) => ({ ...s, link: markBrowserUnsupported(s.link) }));
      },
      onRemoteStream: (stream) => {
        this.remoteStream.set(stream);
        opts.onRemoteStream?.(stream);
      },
    });
  }

  get roomId(): string {
    return this.driver.roomId;
  }

  get noiseAlertCount(): number {
    return this.noiseAlerts;
  }

  /** Dev / E2E only. */
  get link(): LinkDriver {
    return this.driver;
  }

  /** Raw core events (alert side effects: sound, vibration, notifications). */
  onLinkEvent(listener: (ev: LinkEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  start(): void {
    this.driver.start();
    this.housekeeping = setInterval(() => this.housekeepingTick(), HOUSEKEEPING_MS);
    this.qualityTimer = setInterval(() => void this.sampleQuality(), QUALITY_WINDOW_MS);
  }

  stop(): void {
    this.driver.stop();
    if (this.housekeeping) clearInterval(this.housekeeping);
    if (this.restoredTimer) clearTimeout(this.restoredTimer);
    if (this.silenceTimer) clearTimeout(this.silenceTimer);
    if (this.qualityTimer) clearInterval(this.qualityTimer);
    this.housekeeping = this.restoredTimer = this.silenceTimer = this.qualityTimer = null;
    this.qualityBase = null;
    this.remoteStream.set(null);
    this.store.update((s) => ({
      ...s,
      link: { ...s.link, phase: "idle", level: 0 },
      listen: "off",
    }));
  }

  /** "Try now": the core retries immediately and resets its backoff. */
  tryNow(): void {
    this.driver.networkAvailable();
  }

  /** "I'm checking": mutes levels 2-3 for 5 minutes. */
  acknowledgeAlarm(): void {
    this.driver.acknowledgeAlarm();
  }

  setListen(mode: ListenMode): void {
    this.store.update((s) => ({ ...s, listen: mode }));
    this.driver.setListen(mode !== "off");
  }

  requestTestAlert(): void {
    this.driver.requestProbe();
  }

  snooze(minutes: number): void {
    const ms = Math.max(1, Math.round(minutes)) * 60_000;
    this.driver.setSnooze(this.driver.now() + ms);
    this.store.update((s) => ({ ...s, snoozedUntilWallMs: this.wallNow() + ms }));
  }

  cancelSnooze(): void {
    this.driver.setSnooze(null);
    this.store.update((s) => ({ ...s, snoozedUntilWallMs: null }));
  }

  stopAlertSound(): void {
    this.store.update((s) => (s.alert ? { ...s, alert: { ...s.alert, soundStopped: true } } : s));
  }

  requestSilenceBaby(): void {
    this.driver.requestQuiet();
    this.store.update((s) => ({ ...s, silenceRequest: "pending" }));
    if (this.silenceTimer) clearTimeout(this.silenceTimer);
    this.silenceTimer = setTimeout(() => {
      this.store.update((s) =>
        s.silenceRequest === "pending"
          ? { ...s, silenceRequest: s.peer.silence === "silent" ? "applied" : "failed" }
          : s,
      );
    }, SILENCE_REQUEST_TIMEOUT_MS);
  }

  dismissMissedEvents(): void {
    this.store.update((s) => ({ ...s, missed: null }));
  }

  dismissSecurityWarning(): void {
    this.store.update((s) => ({ ...s, securityWarning: false }));
  }

  private ctx(): EventContext {
    return { nowElapsedMs: this.driver.now(), nowWallMs: this.wallNow() };
  }

  private onEvent(ev: LinkEvent): void {
    if (ev.type === "noise_alert") this.noiseAlerts += 1;
    this.store.update((s) => {
      let next = reduceParentEvent(s, ev, this.ctx());
      if (ev.type === "listen_changed" && !ev.enabled) next = { ...next, listen: "off" };
      if (
        ev.type === "peer_status" &&
        next.silenceRequest === "pending" &&
        next.peer.silence === "silent"
      ) {
        next = { ...next, silenceRequest: "applied" };
      }
      return this.unverifiable ? { ...next, link: markBrowserUnsupported(next.link) } : next;
    });
    if (ev.type === "link_restored") {
      if (this.restoredTimer) clearTimeout(this.restoredTimer);
      this.restoredTimer = setTimeout(
        () => this.store.update((s) => ({ ...s, restoredAfterMs: null })),
        RESTORED_BANNER_MS,
      );
    }
    for (const listener of this.listeners) listener(ev);
  }

  /** Packet loss of the incoming audio over the last window, for the "Weak connection" chip. */
  private async sampleQuality(): Promise<void> {
    const pc = get(this.store).link.phase === "connected" ? this.driver.currentPeer()?.pc : null;
    let counters: AudioCounters | null = null;
    try {
      counters = pc ? await readInboundAudioCounters(pc) : null;
    } catch {
      counters = null;
    }
    const base = this.qualityBase;
    this.qualityBase = counters;
    const quality = counters && base ? qualityFromWindow(base, counters) : "unknown";
    this.store.update((s) =>
      s.link.quality === quality ? s : { ...s, link: { ...s.link, quality } },
    );
  }

  private housekeepingTick(): void {
    const ctx = this.ctx();
    const current = get(this.store);
    const next = tickParentState(current, ctx);
    if (next !== current) this.store.set(next);
  }
}
