import { afterEach, describe, expect, it, vi } from "vitest";
import { sampleStreamRms } from "./sample-stream-rms.js";

describe("sampleStreamRms", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("returns zero when stream has no audio tracks", async () => {
    const stream = { getAudioTracks: () => [] } as unknown as MediaStream;
    const result = await sampleStreamRms(stream, { windowMs: 10, intervalMs: 5 });
    expect(result).toEqual({ maxRms: 0, avgRms: 0, samples: 0 });
  });

  it("returns zero when AudioContext is unavailable", async () => {
    vi.stubGlobal("window", {});
    const stream = {
      getAudioTracks: () => [{ readyState: "live" }],
    } as unknown as MediaStream;
    const result = await sampleStreamRms(stream);
    expect(result.maxRms).toBe(0);
    expect(result.samples).toBe(0);
  });
});
