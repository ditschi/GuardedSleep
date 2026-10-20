import type { LinkQuality } from "./ui-contract.js";

/** Quality is judged over windows of this length. */
export const QUALITY_WINDOW_MS = 5_000;
/** Fewer packets than this in a window say nothing (audio is not flowing, e.g. not listening). */
const MIN_PACKETS = 10;

export type AudioCounters = { received: number; lost: number };

export function qualityFromWindow(prev: AudioCounters, cur: AudioCounters): LinkQuality {
  const received = Math.max(0, cur.received - prev.received);
  const lost = Math.max(0, cur.lost - prev.lost);
  const total = received + lost;
  if (total < MIN_PACKETS) return "unknown";
  const loss = lost / total;
  if (loss < 0.03) return "good";
  if (loss < 0.1) return "medium";
  return "poor";
}

type StatsEntry = { type?: string; kind?: string; mediaType?: string; [key: string]: unknown };

/** Sum of the inbound audio RTP counters of a peer connection; null if it has none (yet). */
export async function readInboundAudioCounters(pc: {
  getStats?: () => Promise<unknown>;
}): Promise<AudioCounters | null> {
  if (typeof pc.getStats !== "function") return null;
  const report = (await pc.getStats()) as { forEach(cb: (v: StatsEntry) => void): void };
  let found = false;
  const sum: AudioCounters = { received: 0, lost: 0 };
  report.forEach((e) => {
    if (e.type !== "inbound-rtp" || (e.kind ?? e.mediaType) !== "audio") return;
    found = true;
    if (typeof e.packetsReceived === "number") sum.received += e.packetsReceived;
    if (typeof e.packetsLost === "number") sum.lost += Math.max(0, e.packetsLost);
  });
  return found ? sum : null;
}
