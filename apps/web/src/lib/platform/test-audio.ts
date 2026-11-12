/*
 * Fake Web Audio and media streams for the audio tests. Test-only. Nodes record their connections
 * and parameters; analysers return samples a test queues; oscillators log what was started.
 */

export class FakeParam {
  value = 1;
  ramps: Array<[string, number, number]> = [];
  cancelled: number[] = [];
  cancelScheduledValues(t: number): void {
    this.cancelled.push(t);
  }
  setValueAtTime(v: number, t: number): void {
    this.ramps.push(["set", v, t]);
  }
  linearRampToValueAtTime(v: number, t: number): void {
    this.ramps.push(["ramp", v, t]);
  }
}

export class FakeNode {
  readonly to: FakeNode[] = [];
  disconnected = false;
  connect(node: FakeNode): FakeNode {
    this.to.push(node);
    return node;
  }
  /** Without an argument every connection goes, like `AudioNode.disconnect()`. */
  disconnect(): void {
    this.disconnected = true;
    this.to.length = 0;
  }
}

export class FakeCompressor extends FakeNode {
  threshold = new FakeParam();
  knee = new FakeParam();
  ratio = new FakeParam();
  attack = new FakeParam();
  release = new FakeParam();
}

export class FakeShaper extends FakeNode {
  curve: Float32Array | null = null;
}

export class FakeGain extends FakeNode {
  gain = new FakeParam();
}

export class FakeAnalyser extends FakeNode {
  fftSize = 2048;
  /** What the next `getFloatTimeDomainData` call fills in (cycled). */
  samples: number[] = [];
  getFloatTimeDomainData(buffer: Float32Array): void {
    for (let i = 0; i < buffer.length; i++) buffer[i] = this.samples[i % this.samples.length] ?? 0;
  }
}

export class FakeOscillator extends FakeNode {
  frequency = new FakeParam();
  started: number | null = null;
  stopped: number | null = null;
  onended: (() => void) | null = null;
  start(at = 0): void {
    this.started = at;
  }
  stop(at = 0): void {
    this.stopped = at;
  }
}

export class FakeTrack {
  stopped = false;
  enabled = true;
  constructor(readonly kind = "audio") {}
  stop(): void {
    this.stopped = true;
  }
}

export class FakeStream {
  constructor(readonly tracks: FakeTrack[] = [new FakeTrack()]) {}
  getTracks(): FakeTrack[] {
    return this.tracks;
  }
  getAudioTracks(): FakeTrack[] {
    return this.tracks.filter((t) => t.kind === "audio");
  }
}

export class FakeSource extends FakeNode {
  constructor(readonly stream: FakeStream) {
    super();
  }
}

export class FakeDestination extends FakeNode {
  stream = new FakeStream();
}

export class FakeAudioContext {
  state: "suspended" | "running" = "suspended";
  currentTime = 0;
  destination = new FakeNode();
  resumeCalls = 0;
  allowResume = true;
  readonly analysers: FakeAnalyser[] = [];
  readonly oscillators: FakeOscillator[] = [];
  readonly gains: FakeGain[] = [];
  readonly compressors: FakeCompressor[] = [];
  readonly shapers: FakeShaper[] = [];
  readonly sources: FakeSource[] = [];
  readonly destinations: FakeDestination[] = [];
  private readonly listeners: Array<() => void> = [];

  addEventListener(_type: string, fn: () => void): void {
    this.listeners.push(fn);
  }
  setState(state: "suspended" | "running"): void {
    this.state = state;
    for (const l of this.listeners) l();
  }
  resume(): Promise<void> {
    this.resumeCalls++;
    if (this.allowResume) this.state = "running";
    return Promise.resolve();
  }
  createAnalyser(): FakeAnalyser {
    const n = new FakeAnalyser();
    this.analysers.push(n);
    return n;
  }
  createGain(): FakeGain {
    const n = new FakeGain();
    this.gains.push(n);
    return n;
  }
  createDynamicsCompressor(): FakeCompressor {
    const n = new FakeCompressor();
    this.compressors.push(n);
    return n;
  }
  createWaveShaper(): FakeShaper {
    const n = new FakeShaper();
    this.shapers.push(n);
    return n;
  }
  createOscillator(): FakeOscillator {
    const n = new FakeOscillator();
    this.oscillators.push(n);
    return n;
  }
  createMediaStreamSource(stream: FakeStream): FakeSource {
    const n = new FakeSource(stream);
    this.sources.push(n);
    return n;
  }
  createMediaStreamDestination(): FakeDestination {
    const n = new FakeDestination();
    this.destinations.push(n);
    return n;
  }
}

export const asContext = (c: FakeAudioContext): AudioContext => c as unknown as AudioContext;
export const asStream = (s: FakeStream): MediaStream => s as unknown as MediaStream;
