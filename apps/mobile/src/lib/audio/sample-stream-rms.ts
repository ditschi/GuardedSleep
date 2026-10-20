export type StreamRmsSample = {
  maxRms: number;
  avgRms: number;
  samples: number;
};

export type SampleStreamRmsOptions = {
  /** Total capture window in milliseconds (default 800). */
  windowMs?: number;
  /** Interval between RMS readings in milliseconds (default 50). */
  intervalMs?: number;
};

function rmsFromTimeDomain(buf: Uint8Array): number {
  let sum = 0;
  for (let i = 0; i < buf.length; i++) {
    const n = (buf[i]! - 128) / 128;
    sum += n * n;
  }
  return Math.sqrt(sum / buf.length);
}

/**
 * Samples RMS levels from a live MediaStream (WebRTC remote audio, etc.).
 * Resolves when the capture window elapses or the stream ends.
 */
export async function sampleStreamRms(
  stream: MediaStream,
  opts?: SampleStreamRmsOptions,
): Promise<StreamRmsSample> {
  const windowMs = opts?.windowMs ?? 800;
  const intervalMs = opts?.intervalMs ?? 50;
  const tracks = stream.getAudioTracks();
  if (tracks.length === 0) {
    return { maxRms: 0, avgRms: 0, samples: 0 };
  }

  const AC =
    typeof window !== "undefined"
      ? (window.AudioContext ??
        (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)
      : undefined;
  if (!AC) {
    return { maxRms: 0, avgRms: 0, samples: 0 };
  }

  const ctx = new AC();
  if (ctx.state === "suspended") {
    await ctx.resume().catch(() => {});
  }

  const analyser = ctx.createAnalyser();
  analyser.fftSize = 2048;
  analyser.smoothingTimeConstant = 0.3;
  const src = ctx.createMediaStreamSource(stream);
  src.connect(analyser);
  const buf = new Uint8Array(analyser.fftSize);

  const readings: number[] = [];
  const deadline = Date.now() + windowMs;

  await new Promise<void>((resolve) => {
    const tick = (): void => {
      if (tracks.every((t) => t.readyState === "ended") || Date.now() >= deadline) {
        resolve();
        return;
      }
      analyser.getByteTimeDomainData(buf);
      readings.push(rmsFromTimeDomain(buf));
      window.setTimeout(tick, intervalMs);
    };
    tick();
  });

  src.disconnect();
  await ctx.close().catch(() => {});

  if (readings.length === 0) {
    return { maxRms: 0, avgRms: 0, samples: 0 };
  }
  const maxRms = Math.max(...readings);
  const avgRms = readings.reduce((a, b) => a + b, 0) / readings.length;
  return { maxRms, avgRms, samples: readings.length };
}
