/**
 * Parent-side connection alarm (WebAudio, only after the user unlocked audio with a tap):
 * level 2 = a soft beep every 10 s, level 3 = a continuous tone. Never used on the baby unit.
 */
export const LEVEL2_BEEP_INTERVAL_MS = 10_000;

export type AlarmMode = "off" | "beep" | "continuous";

export function alarmModeFor(level: number, acknowledged: boolean): AlarmMode {
  if (acknowledged || level < 2) return "off";
  return level >= 3 ? "continuous" : "beep";
}

export class AlarmTone {
  private ctx: AudioContext | null = null;
  private mode: AlarmMode = "off";
  private timer: ReturnType<typeof setInterval> | null = null;
  private loop: { osc: OscillatorNode; gain: GainNode } | null = null;

  /** Must run inside a user gesture. Resolves to whether audio can play. */
  async unlock(): Promise<boolean> {
    const AC =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AC) return false;
    this.ctx ??= new AC();
    try {
      await this.ctx.resume();
    } catch {
      return false;
    }
    this.beep(0.12);
    return this.ctx.state === "running";
  }

  get unlocked(): boolean {
    return this.ctx?.state === "running";
  }

  setMode(mode: AlarmMode): void {
    if (mode === this.mode) return;
    this.mode = mode;
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
    this.stopLoop();
    if (mode === "beep") {
      this.beep(0.4);
      this.timer = setInterval(() => this.beep(0.4), LEVEL2_BEEP_INTERVAL_MS);
    } else if (mode === "continuous") {
      this.startLoop();
    }
  }

  destroy(): void {
    this.setMode("off");
    void this.ctx?.close().catch(() => {});
    this.ctx = null;
  }

  private beep(seconds: number): void {
    const ctx = this.ctx;
    if (!ctx || ctx.state !== "running") return;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.frequency.value = 660;
    gain.gain.setValueAtTime(0.0001, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.25, ctx.currentTime + 0.05);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + seconds);
    osc.connect(gain).connect(ctx.destination);
    osc.start();
    osc.stop(ctx.currentTime + seconds + 0.05);
  }

  private startLoop(): void {
    const ctx = this.ctx;
    if (!ctx || ctx.state !== "running") return;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "square";
    osc.frequency.value = 880;
    gain.gain.value = 0.2;
    osc.connect(gain).connect(ctx.destination);
    osc.start();
    this.loop = { osc, gain };
  }

  private stopLoop(): void {
    if (!this.loop) return;
    try {
      this.loop.osc.stop();
    } catch {
      /* already stopped */
    }
    this.loop.osc.disconnect();
    this.loop.gain.disconnect();
    this.loop = null;
  }
}
