/*
 * The browser's I/O for talk-back (`add-talk-back` design sections 7 and 8). Nothing here decides
 * when a talk may play or end: the core grants and times it, these modules only open and close the
 * audio. Two rules shape the baby side (design section 8, Critical Review N3 to N5):
 *
 *  - talk audio reaches the loudspeaker only through the Web Audio graph below, never through a
 *    media element (an element that holds a talk stream stays muted); and
 *  - the end of a lease is scheduled on the audio clock the moment it opens, so a throttled
 *    background timer cannot extend a talk. A platform timer additionally disconnects the graph.
 *
 * No part of the talk path records: there is no `MediaRecorder`, no sink on the parent's capture
 * and no worklet (a test scans the sources for it).
 */
import { RmsMeter, type SoundGate } from "./audio";
import type { Cancel, Timers } from "./clock";
import type { TalkLevel } from "./media-engine";
import {
  TALK_CHIME_FLOOR,
  TALK_CHIME_NOTE_S,
  TALK_CHIME_NOTES_HZ,
  TALK_CHIME_RATIO,
  TALK_LEVEL_GAIN,
  TALK_LIMITER_ATTACK_S,
  TALK_LIMITER_CEILING,
  TALK_LIMITER_RATIO,
  TALK_LIMITER_RELEASE_S,
  TALK_LIMITER_THRESHOLD_DB,
} from "./policy";

// ---- baby: playback of the parent's voice ------------------------------------------------------

export interface TalkSink {
  srcObject: MediaProvider | null;
  muted: boolean;
  play(): Promise<void>;
}

export interface TalkPlayback {
  /** Attach the talk receiver of the live attempt (a new attempt replaces the old one). */
  attach(track: MediaStreamTrack, stream: MediaStream): void;
  /** Open the lease: fade in, and stop by itself `remainingMs` later. */
  open(level: TalkLevel, fadeInMs: number, remainingMs: number): void;
  /** Close the lease and cancel a running chime. Idempotent. */
  close(): void;
  /** The chime through the same limiter; `done` runs when it ended (not when it was cancelled). */
  playChime(level: TalkLevel, done: () => void): void;
  readonly isOpen: boolean;
  /** RMS of the incoming voice since the last call; null when no lease is open. */
  drainRms(): number | null;
  release(): void;
}

export interface TalkPlaybackDeps {
  gate: SoundGate;
  timers: Timers;
  /** Chromium delivers a remote WebRTC stream to Web Audio only while a media element holds it. */
  createSink(): TalkSink;
}

/** A hard clip at `ceiling` (linear): the last stage in front of the loudspeaker. */
function clipCurve(ceiling: number): Float32Array<ArrayBuffer> {
  const points = 2049;
  const curve = new Float32Array(points);
  for (let i = 0; i < points; i++) {
    const x = (i / (points - 1)) * 2 - 1;
    curve[i] = Math.max(-ceiling, Math.min(ceiling, x));
  }
  return curve;
}

export function createTalkPlayback(deps: TalkPlaybackDeps): TalkPlayback {
  const ctx = deps.gate.context();
  // The voice: source -> level/fade gain -> compressor -> clip -> (lease) -> destination.
  const level = ctx.createGain();
  level.gain.value = 0;
  const limiter = ctx.createDynamicsCompressor();
  limiter.threshold.value = TALK_LIMITER_THRESHOLD_DB;
  limiter.knee.value = 0;
  limiter.ratio.value = TALK_LIMITER_RATIO;
  limiter.attack.value = TALK_LIMITER_ATTACK_S;
  limiter.release.value = TALK_LIMITER_RELEASE_S;
  const clip = ctx.createWaveShaper();
  clip.curve = clipCurve(TALK_LIMITER_CEILING);
  level.connect(limiter);
  limiter.connect(clip);

  let routed = false;
  const route = (on: boolean): void => {
    if (on === routed) return;
    routed = on;
    // Disconnected outside a lease or a chime: the graph has no path to the loudspeaker.
    if (on) clip.connect(ctx.destination);
    else clip.disconnect();
  };

  let current: {
    track: MediaStreamTrack;
    source: MediaStreamAudioSourceNode;
    meter: RmsMeter;
    sink: TalkSink;
  } | null = null;
  let leaseOpen = false;
  let leaseEnd: Cancel | null = null;
  let chime: { cancel: Cancel; stop: () => void } | null = null;

  const stopChime = (): void => {
    chime?.cancel();
    chime?.stop();
    chime = null;
  };

  const closeLease = (): void => {
    leaseEnd?.();
    leaseEnd = null;
    const now = ctx.currentTime;
    level.gain.cancelScheduledValues(now);
    level.gain.setValueAtTime(0, now);
    if (current) current.track.enabled = false;
    leaseOpen = false;
    if (chime === null) route(false);
  };

  const detach = (): void => {
    if (!current) return;
    current.track.enabled = false;
    current.meter.stop();
    current.source.disconnect();
    current.sink.srcObject = null;
    current = null;
  };

  return {
    attach(track, stream) {
      detach();
      track.enabled = false;
      const sink = deps.createSink();
      sink.muted = true; // always: the sound goes through Web Audio, the element only keeps the stream alive
      sink.srcObject = stream;
      void sink.play().catch(() => undefined);
      const source = ctx.createMediaStreamSource(stream);
      const analyser = ctx.createAnalyser();
      const meter = new RmsMeter(analyser, deps.timers);
      source.connect(analyser);
      source.connect(level);
      current = { track, source, meter, sink };
    },
    open(talkLevel, fadeInMs, remainingMs) {
      stopChime();
      closeLease();
      const now = ctx.currentTime;
      const remainingS = Math.max(0, remainingMs) / 1000;
      level.gain.setValueAtTime(0, now);
      level.gain.linearRampToValueAtTime(TALK_LEVEL_GAIN[talkLevel], now + fadeInMs / 1000);
      // The stop on the audio clock: a throttled timer cannot make this later.
      level.gain.setValueAtTime(0, now + remainingS);
      if (current) current.track.enabled = true;
      leaseOpen = true;
      route(true);
      leaseEnd = deps.timers.after(closeLease, Math.max(0, remainingMs));
    },
    close() {
      stopChime();
      closeLease();
    },
    playChime(talkLevel, done) {
      stopChime();
      const now = ctx.currentTime;
      const amplitude = Math.max(TALK_LEVEL_GAIN[talkLevel] * TALK_CHIME_RATIO, TALK_CHIME_FLOOR);
      const bus = ctx.createGain();
      bus.gain.value = amplitude;
      bus.connect(limiter);
      const nodes: Array<{ osc: OscillatorNode; env: GainNode }> = [];
      TALK_CHIME_NOTES_HZ.forEach((hz, i) => {
        const osc = ctx.createOscillator();
        const env = ctx.createGain();
        const at = now + i * TALK_CHIME_NOTE_S;
        osc.frequency.value = hz;
        // Slow attack and release: a soft bell, never a click.
        env.gain.setValueAtTime(0, at);
        env.gain.linearRampToValueAtTime(1, at + TALK_CHIME_NOTE_S * 0.3);
        env.gain.linearRampToValueAtTime(0, at + TALK_CHIME_NOTE_S);
        osc.connect(env);
        env.connect(bus);
        osc.start(at);
        osc.stop(at + TALK_CHIME_NOTE_S);
        nodes.push({ osc, env });
      });
      route(true);
      const stop = (): void => {
        for (const n of nodes) {
          n.osc.disconnect();
          n.env.disconnect();
        }
        bus.disconnect();
        if (!leaseOpen) route(false);
      };
      const cancel = deps.timers.after(
        () => {
          chime = null;
          stop();
          done();
        },
        TALK_CHIME_NOTES_HZ.length * TALK_CHIME_NOTE_S * 1000,
      );
      chime = { cancel, stop };
    },
    get isOpen() {
      return leaseOpen;
    },
    drainRms() {
      const rms = current?.meter.drain() ?? null;
      return leaseOpen ? rms : null;
    },
    release() {
      stopChime();
      closeLease();
      detach();
      level.disconnect();
      limiter.disconnect();
      route(false);
    },
  };
}

// ---- parent: the microphone of a talk ----------------------------------------------------------

/** A capture opened at the press and attached only at the grant. */
export interface TalkCapture {
  /** The track once the browser opened the microphone; null when it could not. */
  readonly ready: Promise<MediaStreamTrack | null>;
  /** Stops every track now, also one that is still being opened. Idempotent. */
  stop(): void;
}

/**
 * Opens the parent's microphone with voice processing on (echo cancellation, noise suppression,
 * auto gain): an evened-out voice before the baby's own limiter (design section 4). The track goes
 * straight to the sender; no node and no sink ever touch it.
 */
export function openTalkCapture(
  getUserMedia: (constraints: MediaStreamConstraints) => Promise<MediaStream>,
): TalkCapture {
  let stopped = false;
  let stream: MediaStream | null = null;
  const ready = getUserMedia({
    audio: {
      echoCancellation: true,
      noiseSuppression: true,
      autoGainControl: true,
      channelCount: 1,
    },
    video: false,
  }).then(
    (opened) => {
      if (stopped) {
        for (const t of opened.getTracks()) t.stop();
        return null;
      }
      stream = opened;
      return opened.getAudioTracks()[0] ?? null;
    },
    () => null,
  );
  return {
    ready,
    stop() {
      stopped = true;
      if (!stream) return;
      for (const t of stream.getTracks()) t.stop();
      stream = null;
    },
  };
}

export type MicrophonePermission = "granted" | "denied" | "prompt" | "unknown";

/** The Permissions API answer for the microphone; "unknown" where the browser cannot tell. */
export async function microphonePermission(nav: {
  permissions?: { query(d: { name: string }): Promise<{ state: string }> };
}): Promise<MicrophonePermission> {
  try {
    const status = await nav.permissions?.query({ name: "microphone" });
    if (status?.state === "granted" || status?.state === "denied" || status?.state === "prompt") {
      return status.state;
    }
  } catch {
    // Firefox and Safari reject the name "microphone": the permission step decides instead.
  }
  return "unknown";
}

/**
 * The permission step: asks once and closes the microphone again at once. A browser that offers a
 * one-time permission (Chrome's "this time", Firefox's "now") shows it in this prompt; nothing is
 * kept by this page.
 */
export async function requestMicrophone(
  getUserMedia: (constraints: MediaStreamConstraints) => Promise<MediaStream>,
): Promise<"granted" | "refused"> {
  try {
    const stream = await getUserMedia({ audio: true, video: false });
    for (const t of stream.getTracks()) t.stop();
    return "granted";
  } catch {
    return "refused";
  }
}

/**
 * Whether this browser may talk: it needs a secure context with media devices, and it is not iOS
 * (every iOS browser is WebKit). Alert audio is not proven to keep its output and level after
 * `getUserMedia` there, so talk-back reports "not supported" (spec `web-client`, Critical Review I9).
 */
export function browserTalkSupported(nav: {
  userAgent: string;
  platform?: string;
  maxTouchPoints?: number;
  mediaDevices?: unknown;
}): boolean {
  if (!nav.mediaDevices) return false;
  const ios =
    /iP(hone|ad|od)/.test(nav.userAgent) ||
    // iPadOS reports a Mac with a touch screen.
    (nav.platform === "MacIntel" && (nav.maxTouchPoints ?? 0) > 1);
  return !ios;
}
