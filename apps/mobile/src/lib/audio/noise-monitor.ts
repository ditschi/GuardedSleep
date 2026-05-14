export type NoiseMonitorOptions = {
  /** RMS 0..1; above this counts as "noisy". */
  threshold: number;
  /** How long RMS must stay above threshold (ms). */
  holdMs: number;
  onLoud: (rms: number) => void;
  onQuiet?: () => void;
  /** Sample interval ms (default 100). */
  intervalMs?: number;
};

/**
 * Lightweight RMS gate on a microphone MediaStream (AnalyserNode).
 */
export class NoiseMonitor {
  private ctx: AudioContext | null = null;
  private analyser: AnalyserNode | null = null;
  private buf: Uint8Array | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;
  private aboveSince: number | null = null;
  private loud = false;
  private lastRms = 0;

  constructor(
    private readonly stream: MediaStream,
    private readonly opts: NoiseMonitorOptions,
  ) {}

  getLastRms(): number {
    return this.lastRms;
  }

  /** Update gate while running (e.g. sliders) without restarting capture. */
  setGate(threshold: number, holdMs: number): void {
    this.opts.threshold = threshold;
    this.opts.holdMs = holdMs;
  }

  /** After visibility / audio focus changes (especially Android WebView). */
  resumeAudioContext(): void {
    const c = this.ctx;
    if (c && c.state === "suspended") void c.resume().catch(() => {});
  }

  start(): void {
    this.stop();
    const AC =
      typeof window !== "undefined"
        ? (window.AudioContext ??
          (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)
        : undefined;
    if (!AC) return;
    this.ctx = new AC();
    this.analyser = this.ctx.createAnalyser();
    this.analyser.fftSize = 2048;
    this.analyser.smoothingTimeConstant = 0.85;
    const src = this.ctx.createMediaStreamSource(this.stream);
    src.connect(this.analyser);
    const n = this.analyser.frequencyBinCount;
    this.buf = new Uint8Array(new ArrayBuffer(n));
    const interval = this.opts.intervalMs ?? 100;
    this.timer = setInterval(() => this.tick(), interval);
  }

  stop(): void {
    if (this.timer != null) {
      clearInterval(this.timer);
      this.timer = null;
    }
    void this.ctx?.close().catch(() => {});
    this.ctx = null;
    this.analyser = null;
    this.buf = null;
    this.aboveSince = null;
    this.loud = false;
  }

  private tick(): void {
    const an = this.analyser;
    const buf = this.buf;
    if (!an || !buf) return;
    an.getByteTimeDomainData(buf as unknown as Uint8Array<ArrayBuffer>);
    let sum = 0;
    for (let i = 0; i < buf.length; i++) {
      const x = (buf[i]! - 128) / 128;
      sum += x * x;
    }
    const rms = Math.sqrt(sum / buf.length);
    this.lastRms = rms;

    const now = performance.now();
    const hold = this.opts.holdMs;

    if (rms >= this.opts.threshold) {
      if (this.aboveSince == null) this.aboveSince = now;
      if (!this.loud && now - this.aboveSince >= hold) {
        this.loud = true;
        this.opts.onLoud(rms);
      }
    } else {
      this.aboveSince = null;
      if (this.loud) {
        this.loud = false;
        this.opts.onQuiet?.();
      }
    }
  }
}
