import { writable, type Readable } from "svelte/store";
import type { CoreApi, DeviceStatus, LinkConfig, LinkEvent } from "@guardedsleep/core-wasm";
import { BabyAudio } from "./baby-audio.js";
import {
  computeUnitWarnings,
  mergeBabyDeviceStatus,
  watchBrowserDeviceStatus,
} from "./device-status.js";
import { LinkDriver, type LinkDriverEnv } from "./link-driver.js";
import { markBrowserUnsupported, reduceBabyEvent } from "./monitor-state.js";
import { initialBabyState, type BabyMonitorState } from "./ui-contract.js";

/** The microphone delivers only digital silence for this long: shown as a hint to the baby user. */
const MIC_SILENT_AFTER_MS = 30_000;

/** The wake lock is requested asynchronously; do not warn about it during this start-up grace. */
const WAKE_LOCK_GRACE_MS = 5_000;
const WARNINGS_REFRESH_MS = 5_000;

export type BabyMonitorOptions = {
  core: CoreApi;
  config: LinkConfig;
  signalingUrls: string[];
  /** The captured microphone; the monitor takes ownership and stops it on `stop()`. */
  stream: MediaStream;
  env?: Partial<LinkDriverEnv>;
  wallNow?: () => number;
  /** Replaceable in tests; defaults to the browser battery watcher. */
  watchDeviceStatus?: typeof watchBrowserDeviceStatus;
  onVerified?: () => void;
};

/**
 * One baby monitoring run. The baby never plays audio: the outgoing track is `sendonly`, and no
 * `<audio>` element or destination node is ever connected to the speakers.
 */
export class BabyMonitor {
  private readonly store = writable<BabyMonitorState>(initialBabyState());
  readonly state: Readable<BabyMonitorState> = { subscribe: this.store.subscribe };

  /** Wall time of the last detected suspend/resume gap, with its length; null if none happened. */
  readonly suspendGap = writable<{ atWallMs: number; gapMs: number } | null>(null);

  private readonly driver: LinkDriver;
  private readonly audio: BabyAudio;
  private readonly wallNow: () => number;
  private readonly listeners = new Set<(ev: LinkEvent) => void>();
  private stopDeviceWatch: (() => void) | null = null;
  private zeroSince: number | null = null;
  private verified = false;
  private battery: DeviceStatus = {};
  private pageHidden = false;
  private wakeLockHeld = false;
  private lastGapWallMs: number | null = null;
  private micEnded = false;
  private startedWallMs = 0;
  private lastPublished = "";
  private unverifiable = false;
  private warningsTimer: ReturnType<typeof setInterval> | null = null;
  private micTracks: MediaStreamTrack[] = [];
  private readonly onMicEnded = (): void => {
    this.micEnded = true;
    this.publishStatus();
  };

  constructor(private readonly opts: BabyMonitorOptions) {
    this.wallNow = opts.wallNow ?? (() => Date.now());
    this.audio = new BabyAudio(opts.stream, (rms) => this.onLevel(rms));
    this.driver = new LinkDriver({
      core: opts.core,
      config: opts.config,
      signalingUrls: opts.signalingUrls,
      env: opts.env,
      onTimeJump: (gapMs) => {
        const atWallMs = this.wallNow();
        this.lastGapWallMs = atWallMs;
        this.suspendGap.set({ atWallMs, gapMs });
        this.publishStatus();
      },
      babyAudio: this.audio,
      onEvent: (ev) => this.onEvent(ev),
      onUnverifiable: () => {
        this.unverifiable = true;
        this.store.update((s) => ({ ...s, link: markBrowserUnsupported(s.link) }));
      },
    });
    this.store.update((s) => ({
      ...s,
      threshold: opts.config.noise_threshold,
      holdSeconds: Math.round(opts.config.noise_hold_ms / 1000),
    }));
  }

  get roomId(): string {
    return this.driver.roomId;
  }

  /** Dev / E2E only. */
  get link(): LinkDriver {
    return this.driver;
  }

  /** Dev / E2E only. */
  get babyAudio(): BabyAudio {
    return this.audio;
  }

  onLinkEvent(listener: (ev: LinkEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** The page tells what only it can see; they travel to the parent as `unit_warnings`. */
  setPageSignals(signals: { hidden: boolean; wakeLockHeld: boolean }): void {
    this.pageHidden = signals.hidden;
    this.wakeLockHeld = signals.wakeLockHeld;
    this.publishStatus();
  }

  start(): void {
    this.startedWallMs = this.wallNow();
    this.micTracks = this.opts.stream.getAudioTracks();
    for (const track of this.micTracks) {
      track.addEventListener("ended", this.onMicEnded);
      if (track.readyState === "ended") this.micEnded = true;
    }
    this.audio.start();
    this.stopDeviceWatch = (this.opts.watchDeviceStatus ?? watchBrowserDeviceStatus)((status) => {
      this.battery = status;
      this.publishStatus();
    });
    this.driver.start();
    this.publishStatus();
    this.warningsTimer = setInterval(() => this.publishStatus(), WARNINGS_REFRESH_MS);
  }

  stop(): void {
    this.driver.stop();
    this.stopDeviceWatch?.();
    this.stopDeviceWatch = null;
    if (this.warningsTimer) clearInterval(this.warningsTimer);
    this.warningsTimer = null;
    for (const track of this.micTracks) track.removeEventListener("ended", this.onMicEnded);
    this.micTracks = [];
    this.audio.stop();
    this.store.update((s) => ({ ...s, link: { ...s.link, phase: "idle", level: 0 }, micLevel: 0 }));
  }

  tryNow(): void {
    this.driver.networkAvailable();
  }

  setKioskLocked(locked: boolean): void {
    this.store.update((s) => ({ ...s, kioskLocked: locked }));
  }

  private publishStatus(): void {
    if (!this.driver.isRunning) return;
    const now = this.wallNow();
    const warnings = computeUnitWarnings({
      pageHidden: this.pageHidden,
      wakeLockHeld: this.wakeLockHeld || now - this.startedWallMs < WAKE_LOCK_GRACE_MS,
      lastGapWallMs: this.lastGapWallMs,
      nowWallMs: now,
      charging: this.battery.charging ?? null,
      micEnded: this.micEnded,
    });
    const status = mergeBabyDeviceStatus(this.battery, warnings);
    const key = JSON.stringify(status);
    if (key === this.lastPublished) return;
    this.lastPublished = key;
    this.driver.deviceStatus(status);
  }

  private onLevel(rms: number): void {
    this.driver.audioLevel(rms);
    const now = this.wallNow();
    if (rms === 0) this.zeroSince ??= now;
    else this.zeroSince = null;
    const micSilent = this.zeroSince != null && now - this.zeroSince >= MIC_SILENT_AFTER_MS;
    this.store.update((s) =>
      s.micLevel === rms && s.micSilent === micSilent ? s : { ...s, micLevel: rms, micSilent },
    );
  }

  private onEvent(ev: LinkEvent): void {
    this.store.update((s) => {
      const next = reduceBabyEvent(s, ev, {
        nowElapsedMs: this.driver.now(),
        nowWallMs: this.wallNow(),
      });
      return this.unverifiable ? { ...next, link: markBrowserUnsupported(next.link) } : next;
    });
    if (ev.type === "status" && ev.status.state === "connected" && !this.verified) {
      this.verified = true;
      this.opts.onVerified?.();
    }
    for (const listener of this.listeners) listener(ev);
  }
}
