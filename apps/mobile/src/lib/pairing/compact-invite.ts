import { normalizeRoomId } from "$lib/utils/room.js";
import { validateSignalingUrl } from "$lib/signaling-url.js";

export type CompactInviteV1 = {
  v: 1;
  r: string;
  s: string;
  /** PSK, base64url (no padding), 32 bytes recommended */
  k: string;
  t: number;
};

export function generatePskBase64Url(): string {
  const bytes = new Uint8Array(32);
  crypto.getRandomValues(bytes);
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]!);
  const b64 = btoa(bin);
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function decodeBase64UrlToBytes(s: string): Uint8Array | null {
  try {
    let t = s.replace(/-/g, "+").replace(/_/g, "/");
    while (t.length % 4) t += "=";
    const bin = atob(t);
    const out = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out;
  } catch {
    return null;
  }
}

export function buildCompactInviteJson(
  room: string,
  signal: string,
  k: string,
  t: number = Date.now(),
): string {
  const payload: CompactInviteV1 = { v: 1, r: room.trim(), s: signal.trim(), k, t };
  return JSON.stringify(payload);
}

export function parseCompactInviteJson(raw: string): CompactInviteV1 | null {
  try {
    const v = JSON.parse(raw) as unknown;
    if (!v || typeof v !== "object") return null;
    const o = v as Record<string, unknown>;
    if (o.v !== 1) return null;
    if (typeof o.r !== "string" || typeof o.s !== "string" || typeof o.k !== "string") return null;
    if (typeof o.t !== "number") return null;
    const room = normalizeRoomId(o.r);
    if (!room || validateSignalingUrl(o.s) !== null) return null;
    if (decodeBase64UrlToBytes(o.k) == null) return null;
    return { v: 1, r: room, s: o.s, k: o.k, t: o.t };
  } catch {
    return null;
  }
}
