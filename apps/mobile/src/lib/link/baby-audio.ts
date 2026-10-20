import type { BabyAudioPort } from "./link-driver.js";

export const PROBE_TONE_HZ = 1000;
const LEVEL_INTERVAL_MS = 100;
const ANALYSER_FFT = 2048;

type AudioContextCtor = typeof AudioContext;

function audioContextCtor(): AudioContextCtor | undefined {
  if (typeof window === "undefined") return undefined;
  return (
    window.AudioContext ??
    (window as unknown as { webkitAudioContext?: AudioContextCtor }).webkitAudioContext
  );
}

/** RMS of a time-domain float buffer (-1..1). */
export function rmsOf(samples: Float32Array): number {
  if (samples.length === 0) return 0;
  let sum = 0;
  for (let i = 0; i < samples.length; i++) sum += samples[i]! * samples[i]!;
  return Math.sqrt(sum / samples.length);
}

/**
 * Baby microphone chain. Analysis taps the live capture track; the outgoing track is a *clone*
 * that stays disabled until the core asks for audio. Nothing here ever reaches the speakers:
 * the analyser and the tone destination are not connected to `ctx.destination`.
 */
export class BabyAudio implements BabyAudioPort {
  readonly outgoingTrack: MediaStreamTrack | null;
  private ctx: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private samples: Float32Array<ArrayBuffer> | null = null;
  private levelTimer: ReturnType<typeof setInterval> | null = null;
  private lastRms = 0;
  private injected: { rms: number; until: number } | null = null;
  /** Dev / E2E only: ignore the microphone for the level meter unless a level is injected. */
  syntheticOnly = false;
  private toneTimer: ReturnType<typeof setTimeout> | null = null;
  private toneStop: (() => void) | null = null;
  private toneTrack: MediaStreamTrack | null = null;

  constructor(
    private readonly stream: MediaStream,
    private readonly onLevel: (rms: number) => void,
  ) {
    const track = stream.getAudioTracks()[0] ?? null;
    this.outgoingTrack = track ? track.clone() : null;
    if (this.outgoingTrack) this.outgoingTrack.enabled = false;
  }

  start(): void {
    const AC = audioContextCtor();
    if (!AC || this.ctx) return;
    this.ctx = new AC();
    void this.ctx.resume().catch(() => {});
    this.analyser = this.ctx.createAnalyser();
    this.analyser.fftSize = ANALYSER_FFT;
    this.ctx.createMediaStreamSource(this.stream).connect(this.analyser);
    this.samples = new Float32Array(new ArrayBuffer(this.analyser.fftSize * 4));
    this.levelTimer = setInterval(() => this.sample(), LEVEL_INTERVAL_MS);
  }

  get currentRms(): number {
    return this.lastRms;
  }

  /** Dev / E2E only: report a synthetic level instead of the microphone for a while. */
  inject(rms: number, durationMs: number): void {
    this.injected = { rms, until: Date.now() + durationMs };
  }

  private sample(): void {
    let rms = this.lastRms;
    if (this.injected && Date.now() < this.injected.until) {
      rms = this.injected.rms;
    } else if (this.syntheticOnly) {
      this.injected = null;
      rms = 0;
    } else if (this.analyser && this.samples) {
      this.injected = null;
      this.analyser.getFloatTimeDomainData(this.samples);
      rms = rmsOf(this.samples);
    }
    this.lastRms = rms;
    this.onLevel(rms);
  }

  setMicEnabled(enabled: boolean): void {
    if (this.outgoingTrack) this.outgoingTrack.enabled = enabled;
  }

  outgoingEnabled(): boolean | null {
    return this.outgoingTrack ? this.outgoingTrack.enabled : null;
  }

  /** Mutes everything the page captured; call when monitoring stops. */
  stop(): void {
    if (this.levelTimer) clearInterval(this.levelTimer);
    this.levelTimer = null;
    this.endTone(null);
    this.outgoingTrack?.stop();
    this.stream.getTracks().forEach((t) => t.stop());
    void this.ctx?.close().catch(() => {});
    this.ctx = null;
    this.analyser = null;
  }

  playProbeTone(durationMs: number, sender: RTCRtpSender | null): void {
    if (!this.ctx || !sender) return;
    this.endTone(null);
    const ctx = this.ctx;
    const destination = ctx.createMediaStreamDestination();
    const osc = ctx.createOscillator();
    osc.type = "sine";
    osc.frequency.value = PROBE_TONE_HZ;
    const gain = ctx.createGain();
    gain.gain.value = 0.5;
    osc.connect(gain);
    gain.connect(destination);
    const track = destination.stream.getAudioTracks()[0];
    if (!track) return;
    this.toneTrack = track;
    this.toneStop = () => {
      try {
        osc.stop();
      } catch {
        /* never started or already stopped */
      }
      osc.disconnect();
      gain.disconnect();
      track.stop();
    };
    osc.start();
    void sender.replaceTrack(track).catch(() => {});
    this.toneTimer = setTimeout(() => this.endTone(sender), durationMs);
  }

  private endTone(sender: RTCRtpSender | null): void {
    if (this.toneTimer) clearTimeout(this.toneTimer);
    this.toneTimer = null;
    if (sender && this.outgoingTrack) void sender.replaceTrack(this.outgoingTrack).catch(() => {});
    this.toneStop?.();
    this.toneStop = null;
    this.toneTrack = null;
  }
}
