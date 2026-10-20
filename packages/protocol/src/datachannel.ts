import { PROTOCOL_VERSION } from "./messages.js";

/** Application messages on the WebRTC DataChannel (JSON text). */
export type DataChannelListen = {
  type: "listen";
  v: typeof PROTOCOL_VERSION;
  enabled: boolean;
};

export type DataChannelNoiseAlert = {
  type: "noise_alert";
  v: typeof PROTOCOL_VERSION;
  rms: number;
  t: number;
  /** Parent probe test — same alert chain as real noise without waiting for a cry. */
  probe?: boolean;
};

/** Parent asks baby unit to run one synthetic alert burst (probe / pre-flight test). */
export type DataChannelProbeTest = {
  type: "probe_test";
  v: typeof PROTOCOL_VERSION;
  t: number;
};

/** Mutual proof both peers share the QR PSK and the same DTLS-SRTP session (sorted SDP fingerprints). */
export type DataChannelPairProof = {
  type: "pair_proof";
  v: typeof PROTOCOL_VERSION;
  /** Lowercase hex HMAC-SHA256 output (64 chars). */
  mac: string;
};

/** Parent asks baby unit to open in-app settings (trusted same as listen). */
export type DataChannelOpenSettings = {
  type: "open_settings";
  v: typeof PROTOCOL_VERSION;
};

export function encodeListen(enabled: boolean): string {
  const msg: DataChannelListen = { type: "listen", v: PROTOCOL_VERSION, enabled };
  return JSON.stringify(msg);
}

export function encodeNoiseAlert(
  rms: number,
  t: number = Date.now(),
  opts?: { probe?: boolean },
): string {
  const msg: DataChannelNoiseAlert = {
    type: "noise_alert",
    v: PROTOCOL_VERSION,
    rms,
    t,
    ...(opts?.probe ? { probe: true } : {}),
  };
  return JSON.stringify(msg);
}

export function encodeProbeTest(t: number = Date.now()): string {
  const msg: DataChannelProbeTest = { type: "probe_test", v: PROTOCOL_VERSION, t };
  return JSON.stringify(msg);
}

export function encodePairProof(macHex: string): string {
  const msg: DataChannelPairProof = { type: "pair_proof", v: PROTOCOL_VERSION, mac: macHex };
  return JSON.stringify(msg);
}

export function encodeOpenSettings(): string {
  const msg: DataChannelOpenSettings = { type: "open_settings", v: PROTOCOL_VERSION };
  return JSON.stringify(msg);
}

export function parseListenCommand(raw: string): boolean | undefined {
  try {
    const v = JSON.parse(raw) as unknown;
    if (!v || typeof v !== "object") return undefined;
    const o = v as Record<string, unknown>;
    if (o.type !== "listen") return undefined;
    if (o.v !== PROTOCOL_VERSION) return undefined;
    if (typeof o.enabled !== "boolean") return undefined;
    return o.enabled;
  } catch {
    return undefined;
  }
}

export function parsePairProof(raw: string): string | undefined {
  try {
    const v = JSON.parse(raw) as unknown;
    if (!v || typeof v !== "object") return undefined;
    const o = v as Record<string, unknown>;
    if (o.type !== "pair_proof") return undefined;
    if (o.v !== PROTOCOL_VERSION) return undefined;
    if (typeof o.mac !== "string" || !/^[0-9a-f]{64}$/i.test(o.mac)) return undefined;
    return o.mac.toLowerCase();
  } catch {
    return undefined;
  }
}

export function parseOpenSettings(raw: string): boolean {
  try {
    const v = JSON.parse(raw) as unknown;
    if (!v || typeof v !== "object") return false;
    const o = v as Record<string, unknown>;
    if (o.type !== "open_settings") return false;
    if (o.v !== PROTOCOL_VERSION) return false;
    return true;
  } catch {
    return false;
  }
}

export function parseNoiseAlert(
  raw: string,
): { rms: number; t: number; probe?: boolean } | undefined {
  try {
    const v = JSON.parse(raw) as unknown;
    if (!v || typeof v !== "object") return undefined;
    const o = v as Record<string, unknown>;
    if (o.type !== "noise_alert") return undefined;
    if (o.v !== PROTOCOL_VERSION) return undefined;
    if (typeof o.rms !== "number" || typeof o.t !== "number") return undefined;
    return {
      rms: o.rms,
      t: o.t,
      ...(o.probe === true ? { probe: true } : {}),
    };
  } catch {
    return undefined;
  }
}

export function parseProbeTest(raw: string): { t: number } | undefined {
  try {
    const v = JSON.parse(raw) as unknown;
    if (!v || typeof v !== "object") return undefined;
    const o = v as Record<string, unknown>;
    if (o.type !== "probe_test") return undefined;
    if (o.v !== PROTOCOL_VERSION) return undefined;
    if (typeof o.t !== "number") return undefined;
    return { t: o.t };
  } catch {
    return undefined;
  }
}
