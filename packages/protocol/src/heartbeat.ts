import { PROTOCOL_VERSION } from "./messages.js";

export type HeartbeatPayload = {
  v: typeof PROTOCOL_VERSION;
  /** Monotonic-ish timestamp (ms since epoch). */
  t: number;
  /** Sequence number for gap detection. */
  seq: number;
  /** Baby unit: normalized 0..1 (e.g. `level * 100` from Battery Manager). */
  battery?: number;
  /** Baby unit: last noise RMS 0..1 for parent volume UI. */
  rms?: number;
};

export type HeartbeatEncodeOpts = {
  battery?: number;
  rms?: number;
};

export function encodeHeartbeat(seq: number, opts?: HeartbeatEncodeOpts): string {
  const payload: HeartbeatPayload = {
    v: PROTOCOL_VERSION,
    t: Date.now(),
    seq,
    ...(opts?.battery !== undefined ? { battery: opts.battery } : {}),
    ...(opts?.rms !== undefined ? { rms: opts.rms } : {}),
  };
  return JSON.stringify({ type: "heartbeat", ...payload });
}

export function parseHeartbeat(raw: string): HeartbeatPayload | undefined {
  try {
    const v = JSON.parse(raw) as unknown;
    if (!v || typeof v !== "object") return undefined;
    const o = v as Record<string, unknown>;
    if (o.type !== "heartbeat") return undefined;
    if (o.v !== PROTOCOL_VERSION) return undefined;
    if (typeof o.t !== "number" || typeof o.seq !== "number") return undefined;
    let battery: number | undefined;
    if (o.battery !== undefined) {
      if (typeof o.battery !== "number" || o.battery < 0 || o.battery > 1) return undefined;
      battery = o.battery;
    }
    let rms: number | undefined;
    if (o.rms !== undefined) {
      if (typeof o.rms !== "number" || o.rms < 0 || o.rms > 1) return undefined;
      rms = o.rms;
    }
    return {
      v: PROTOCOL_VERSION,
      t: o.t,
      seq: o.seq,
      ...(battery !== undefined ? { battery } : {}),
      ...(rms !== undefined ? { rms } : {}),
    };
  } catch {
    return undefined;
  }
}
