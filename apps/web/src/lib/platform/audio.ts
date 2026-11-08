import type { Cancel, Timers } from "./clock";
import type { AlertSink, LinkSnapshot, NoiseKind, Notice } from "./alert-sink";
import { LEVEL2_TONE_INTERVAL_MS } from "./policy";

/** How often the analysers are read; shorter than their window so no audio is skipped. */
const METER_SAMPLE_MS = 20;
const METER_FFT_SIZE = 1024;
const PROBE_FREQ_HZ = 1000;
const PROBE_AMPLITUDE = 0.4;
/** One cycle of the level-3 alarm: two 400 ms tones with gaps. */
const ALARM_CYCLE_MS = 1_200;

/**
 * RMS over everything seen since the last `drain` (0..1). An `AnalyserNode` only exposes its latest
 * window, so it is read faster than the window length and the squares are summed; `drain` then
 * covers the whole interval, like the Android capture tap.
 */
export class RmsMeter {
  private sum = 0;
  private count = 0;
  private readonly buffer: Float32Array<ArrayBuffer>;
  private cancel: Cancel;

  constructor(
    private readonly analyser: AnalyserNode,
    timers: Timers,
  ) {
    analyser.fftSize = METER_FFT_SIZE;
    this.buffer = new Float32Array(analyser.fftSize);
    this.cancel = timers.every(() => this.sample(), METER_SAMPLE_MS);
  }

  sample(): void {
    this.analyser.getFloatTimeDomainData(this.buffer);
    let sum = 0;
    for (const x of this.buffer) sum += x * x;
    this.sum += sum;
    this.count += this.buffer.length;
  }

  /** Null when nothing was sampled since the previous call (so the core is not fed zeros). */
  drain(): number | null {
    if (this.count === 0) return null;
    const rms = Math.sqrt(this.sum / this.count);
    this.sum = 0;
    this.count = 0;
    return Math.min(1, rms);
  }

  stop(): void {
    this.cancel();
  }
}

/**
 * The page's audio context and the user-gesture gate in front of it. Browsers keep a context
 * suspended until a gesture: `unlock()` is called from click handlers (and by the controller's
 * own gesture entry points), and nothing is audible, no tone and no baby audio, before it ran.
 */
export interface SoundGate {
  context(): AudioContext;
  /** Output stage shared by playback and tones; carries the alert gain. */
  master(): GainNode;
  /** Resume the context; call it synchronously from a user gesture. */
  unlock(): Promise<boolean>;
  readonly unlocked: boolean;
  /** Called when `unlocked` changes. */
  onChange(listener: (unlocked: boolean) => void): Cancel;
}

export function createSoundGate(create: () => AudioContext): SoundGate {
  let ctx: AudioContext | null = null;
  let masterNode: GainNode | null = null;
  const listeners = new Set<(unlocked: boolean) => void>();
  let unlocked = false;
  // A context that stops running (phone call, tab freeze) must be unlocked again by a gesture.
  let unlockedOnce = false;

  const set = (value: boolean): void => {
    if (value === unlocked) return;
    unlocked = value;
    for (const l of listeners) l(value);
  };
  const context = (): AudioContext => {
    if (!ctx) {
      const made = create();
      ctx = made;
      made.addEventListener("statechange", () => {
        if (made.state !== "running") set(false);
        else if (unlockedOnce) set(true);
      });
    }
    return ctx;
  };

  return {
    context,
    master() {
      if (!masterNode) {
        const c = context();
        masterNode = c.createGain();
        masterNode.connect(c.destination);
      }
      return masterNode;
    },
    async unlock() {
      const c = context();
      try {
        await c.resume();
      } catch {
        return false;
      }
      if (c.state !== "running") return false;
      unlockedOnce = true;
      set(true);
      return true;
    },
    get unlocked() {
      return unlocked;
    },
    onChange(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** Baby: microphone capture with a gate in front of the outgoing track. */
export interface MicCapture {
  /** The outgoing stream (its track is `sendonly` in the offer). */
  readonly stream: MediaStream;
  readonly track: MediaStreamTrack;
  /** Gate the outgoing audio; closed until the core opens it for noise, listen or probe. */
  setEnabled(enabled: boolean): void;
  /** Mix a test tone into the outgoing stream; never played locally, never in the level. */
  playProbeTone(durationMs: number): void;
  /** RMS of the real microphone since the last call; null when none. */
  drainRms(): number | null;
  release(): void;
}

export interface MicDeps {
  getUserMedia(constraints: MediaStreamConstraints): Promise<MediaStream>;
  gate: SoundGate;
  timers: Timers;
}

/**
 * Opens the microphone. Rejects when the permission is refused or no input exists; the caller
 * shows that as a failure that needs the user. Echo cancellation, noise suppression and auto gain
 * are off: they would change the level the core's noise gate looks at.
 */
export async function openMicCapture(deps: MicDeps): Promise<MicCapture> {
  const mic = await deps.getUserMedia({
    audio: {
      echoCancellation: false,
      noiseSuppression: false,
      autoGainControl: false,
      channelCount: 1,
    },
    video: false,
  });
  const ctx = deps.gate.context();
  const source = ctx.createMediaStreamSource(mic);
  const analyser = ctx.createAnalyser();
  const meter = new RmsMeter(analyser, deps.timers);
  const gate = ctx.createGain();
  gate.gain.value = 0;
  const destination = ctx.createMediaStreamDestination();
  source.connect(analyser);
  source.connect(gate);
  gate.connect(destination);
  const [track] = destination.stream.getAudioTracks();
  if (!track) throw new Error("no outgoing audio track");
  let probeStop: Cancel | null = null;

  return {
    stream: destination.stream,
    track,
    setEnabled(enabled) {
      gate.gain.value = enabled ? 1 : 0;
    },
    playProbeTone(durationMs) {
      probeStop?.();
      const osc = ctx.createOscillator();
      const level = ctx.createGain();
      osc.frequency.value = PROBE_FREQ_HZ;
      level.gain.value = PROBE_AMPLITUDE;
      osc.connect(level);
      level.connect(destination);
      osc.start();
      const end = (): void => {
        osc.stop();
        osc.disconnect();
        level.disconnect();
        probeStop = null;
      };
      const cancel = deps.timers.after(end, durationMs);
      probeStop = () => {
        cancel();
        end();
      };
    },
    drainRms: () => meter.drain(),
    release() {
      probeStop?.();
      meter.stop();
      source.disconnect();
      gate.disconnect();
      for (const t of mic.getTracks()) t.stop();
    },
  };
}

/** Parent: playout of the received baby audio, with a mute ("Stop sound") and a gain. */
export interface RemotePlayback {
  /** Attach the received stream (once per attempt; a new attempt replaces the old one). */
  play(stream: MediaStream): void;
  setMuted(muted: boolean): void;
  /** Gain in percent for everything this device plays (25..150). */
  setGainPct(pct: number): void;
  drainRms(): number | null;
  release(): void;
}

export interface PlaybackDeps {
  gate: SoundGate;
  timers: Timers;
  /** Chromium delivers a remote WebRTC stream to Web Audio only while a media element holds it. */
  createSink(): { srcObject: MediaProvider | null; muted: boolean; play(): Promise<void> };
}

export function createRemotePlayback(deps: PlaybackDeps): RemotePlayback {
  const ctx = deps.gate.context();
  const analyser = ctx.createAnalyser();
  const meter = new RmsMeter(analyser, deps.timers);
  const mute = ctx.createGain();
  analyser.connect(mute);
  mute.connect(deps.gate.master());
  let current: {
    source: MediaStreamAudioSourceNode;
    sink: { srcObject: MediaProvider | null };
  } | null = null;

  const detach = (): void => {
    if (!current) return;
    current.source.disconnect();
    current.sink.srcObject = null;
    current = null;
  };

  return {
    play(stream) {
      detach();
      const sink = deps.createSink();
      sink.muted = true; // the sound goes through Web Audio; the element only keeps the stream alive
      sink.srcObject = stream;
      void sink.play().catch(() => undefined);
      const source = ctx.createMediaStreamSource(stream);
      source.connect(analyser);
      current = { source, sink };
    },
    setMuted(value) {
      mute.gain.value = value ? 0 : 1;
    },
    setGainPct(pct) {
      deps.gate.master().gain.value = Math.min(150, Math.max(25, pct)) / 100;
    },
    drainRms: () => meter.drain(),
    release() {
      detach();
      meter.stop();
      mute.disconnect();
    },
  };
}

interface TonePlan {
  /** A soft tone every this many ms (level 2). */
  toneEveryMs: number | null;
  /** The two-tone alarm loops (level 3). */
  continuousAlarm: boolean;
}

/**
 * What the parent device sounds for the current connection level (ADR-017 section 7). Level 1 is
 * visual only; level 2 repeats a soft tone; level 3 loops the alarm; "I'm checking" mutes both.
 * A terminal failure of a link that worked before is never silent.
 */
export function tonePlanForLink(snapshot: LinkSnapshot): TonePlan {
  const none: TonePlan = { toneEveryMs: null, continuousAlarm: false };
  const { phase, level, acknowledged } = snapshot;
  if (phase === "failed_needs_user" && snapshot.wasConnected) {
    return tonePlanForLink({ ...snapshot, phase: "reconnecting", level: 2 });
  }
  if (phase !== "connecting" && phase !== "reconnecting") return none;
  if (level <= 1 || acknowledged) return none;
  if (level === 2) return { toneEveryMs: LEVEL2_TONE_INTERVAL_MS, continuousAlarm: false };
  return { toneEveryMs: null, continuousAlarm: true };
}

export interface AlertPlayerDeps {
  gate: SoundGate;
  timers: Timers;
}

/**
 * Parent alert tones. The baby's own audio is the noise alert (parent-alerts spec), so a noise
 * event adds no tone; tones are for the connection levels, a stopped baby device and the local
 * test. Nothing sounds before the unlock gesture; a loop that began before it starts sounding at
 * its next beat after the unlock.
 */
export class AlertPlayer implements AlertSink {
  private plan: TonePlan = { toneEveryMs: null, continuousAlarm: false };
  private toneLoop: Cancel | null = null;
  private alarmLoop: Cancel | null = null;

  constructor(private readonly deps: AlertPlayerDeps) {}

  linkState(snapshot: LinkSnapshot): void {
    const next = tonePlanForLink(snapshot);
    if (next.toneEveryMs !== this.plan.toneEveryMs) {
      this.toneLoop?.();
      this.toneLoop = null;
      if (next.toneEveryMs !== null) {
        this.softTone();
        this.toneLoop = this.deps.timers.every(() => this.softTone(), next.toneEveryMs);
      }
    }
    if (next.continuousAlarm !== this.plan.continuousAlarm) {
      this.alarmLoop?.();
      this.alarmLoop = null;
      if (next.continuousAlarm) {
        this.alarmCycle();
        this.alarmLoop = this.deps.timers.every(() => this.alarmCycle(), ALARM_CYCLE_MS);
      }
    }
    this.plan = next;
  }

  linkLost(): void {}

  noiseAlert(_kind: NoiseKind, probe: boolean, _snoozed: boolean): void {
    // The baby's audio is the alert. A test alert without a link has no audio: play the bundled
    // test tone so the chain can still be heard on this device.
    if (probe) this.testTone();
  }

  noiseEnded(): void {}

  stopNoiseSound(): void {}

  notice(notice: Notice): void {
    // One soft tone, no loop: the baby device stopped on purpose.
    if (notice.type === "peer_stopped") this.softTone();
  }

  clear(): void {
    this.toneLoop?.();
    this.alarmLoop?.();
    this.toneLoop = this.alarmLoop = null;
    this.plan = { toneEveryMs: null, continuousAlarm: false };
  }

  /** The test tone of a local test alert. */
  testTone(): void {
    this.tone(PROBE_FREQ_HZ, 0.4, 0.8, 0.05);
  }

  private softTone(): void {
    this.tone(660, 0.35, 0.45, 0.08);
  }

  private alarmCycle(): void {
    this.tone(880, 0.8, 0.4, 0.02, 0);
    this.tone(660, 0.8, 0.4, 0.02, 0.6);
  }

  private tone(freq: number, amplitude: number, seconds: number, fade: number, delay = 0): void {
    if (!this.deps.gate.unlocked) return;
    const ctx = this.deps.gate.context();
    const osc = ctx.createOscillator();
    const env = ctx.createGain();
    const t0 = ctx.currentTime + delay;
    osc.frequency.value = freq;
    // A short fade in and out so a tone never clicks.
    env.gain.setValueAtTime(0, t0);
    env.gain.linearRampToValueAtTime(amplitude, t0 + fade);
    env.gain.setValueAtTime(amplitude, t0 + seconds - fade);
    env.gain.linearRampToValueAtTime(0, t0 + seconds);
    osc.connect(env);
    env.connect(this.deps.gate.master());
    osc.start(t0);
    osc.stop(t0 + seconds);
    osc.onended = () => {
      osc.disconnect();
      env.disconnect();
    };
  }
}
