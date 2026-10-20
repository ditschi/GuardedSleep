import { describe, expect, it } from "vitest";
import { qualityFromWindow, readInboundAudioCounters } from "./link-quality.js";

const w = (received: number, lost: number) =>
  qualityFromWindow({ received: 0, lost: 0 }, { received, lost });

describe("qualityFromWindow", () => {
  it("is good below 3 % loss", () => expect(w(980, 20)).toBe("good"));
  it("is medium from 3 % below 10 %", () => {
    expect(w(970, 30)).toBe("medium");
    expect(w(910, 90)).toBe("medium");
  });
  it("is poor from 10 %", () => expect(w(900, 100)).toBe("poor"));
  it("says nothing when no audio flowed", () => expect(w(3, 0)).toBe("unknown"));
  it("uses the window delta, not the totals", () => {
    expect(qualityFromWindow({ received: 5000, lost: 900 }, { received: 5250, lost: 900 })).toBe(
      "good",
    );
  });
});

describe("readInboundAudioCounters", () => {
  it("sums inbound audio rtp entries and ignores video and outbound", async () => {
    const stats = new Map<string, Record<string, unknown>>([
      ["a", { type: "inbound-rtp", kind: "audio", packetsReceived: 100, packetsLost: 4 }],
      ["v", { type: "inbound-rtp", kind: "video", packetsReceived: 999, packetsLost: 99 }],
      ["o", { type: "outbound-rtp", kind: "audio", packetsSent: 5 }],
    ]);
    expect(await readInboundAudioCounters({ getStats: async () => stats })).toEqual({
      received: 100,
      lost: 4,
    });
  });

  it("returns null without an inbound stream or getStats", async () => {
    expect(await readInboundAudioCounters({ getStats: async () => new Map() })).toBeNull();
    expect(await readInboundAudioCounters({})).toBeNull();
  });
});
