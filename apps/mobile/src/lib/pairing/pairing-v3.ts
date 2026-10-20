import type { PairingError } from "$lib/link/ui-contract.js";
import { validateSignalingUrl } from "$lib/signaling-url.js";

/** Pairing code validity (alpha-ux-flows §3.3): a fresh PSK per code, valid for 5 minutes. */
export const PAIRING_CODE_TTL_SECONDS = 300;
export const PAIRING_PSK_BYTES = 32;
const MAX_SIGNALING_URLS = 4;

/** `{"v":3,"k":<psk base64url>,"s":[signaling URLs],"x":<expiry, epoch seconds>}` */
export type PairingCodeV3 = {
  v: 3;
  k: string;
  s: string[];
  x: number;
};

export type PairingMaterial = {
  pskB64Url: string;
  signalingUrls: string[];
};

export type PairingCodeResult =
  | { ok: true; material: PairingMaterial; expiresAtSeconds: number }
  | { ok: false; error: PairingError };

export function encodeBase64Url(bytes: Uint8Array): string {
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

export function decodeBase64Url(s: string): Uint8Array | null {
  if (!/^[A-Za-z0-9_-]*$/.test(s)) return null;
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

export function generatePsk(): string {
  const bytes = new Uint8Array(PAIRING_PSK_BYTES);
  crypto.getRandomValues(bytes);
  return encodeBase64Url(bytes);
}

export type GeneratedPairingCode = {
  material: PairingMaterial;
  payload: string;
  expiresAtWallMs: number;
};

/** Baby: a fresh PSK and its code. The payload is the only place the PSK leaves memory. */
export function generatePairingCode(
  signalingUrls: string[],
  nowWallMs: number,
): GeneratedPairingCode {
  const urls = signalingUrls
    .filter((u) => validateSignalingUrl(u) === null)
    .slice(0, MAX_SIGNALING_URLS);
  const psk = generatePsk();
  const expiresAtSeconds = Math.floor(nowWallMs / 1000) + PAIRING_CODE_TTL_SECONDS;
  const code: PairingCodeV3 = { v: 3, k: psk, s: urls, x: expiresAtSeconds };
  return {
    material: { pskB64Url: psk, signalingUrls: urls },
    payload: JSON.stringify(code),
    expiresAtWallMs: expiresAtSeconds * 1000,
  };
}

/**
 * Parent: validates a scanned or pasted code. Error mapping follows the shared `PairingError` set:
 * text that is not JSON, or JSON without our shape, is `not_our_code`; JSON cut off in the middle
 * (a partial paste) is `incomplete_paste`.
 */
export function parsePairingCode(text: string, nowWallMs: number): PairingCodeResult {
  const raw = text.trim();
  if (raw === "") return { ok: false, error: "incomplete_paste" };
  if (!raw.startsWith("{")) return { ok: false, error: "not_our_code" };
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return { ok: false, error: looksTruncated(raw) ? "incomplete_paste" : "not_our_code" };
  }
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return { ok: false, error: "not_our_code" };
  }
  const o = value as Record<string, unknown>;
  if (o.v !== 3) return { ok: false, error: "not_our_code" };
  if (typeof o.k !== "string" || !Array.isArray(o.s) || typeof o.x !== "number") {
    return { ok: false, error: "incomplete_paste" };
  }
  const key = decodeBase64Url(o.k);
  if (!key || key.length !== PAIRING_PSK_BYTES) return { ok: false, error: "incomplete_paste" };
  const urls = o.s.filter((u): u is string => typeof u === "string");
  if (
    urls.length === 0 ||
    urls.length !== o.s.length ||
    urls.length > MAX_SIGNALING_URLS ||
    urls.some((u) => validateSignalingUrl(u) !== null)
  ) {
    return { ok: false, error: "not_our_code" };
  }
  if (!Number.isFinite(o.x) || o.x * 1000 <= nowWallMs) return { ok: false, error: "expired" };
  return {
    ok: true,
    material: { pskB64Url: o.k, signalingUrls: urls },
    expiresAtSeconds: o.x,
  };
}

function looksTruncated(raw: string): boolean {
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (const ch of raw) {
    if (inString) {
      if (escaped) escaped = false;
      else if (ch === "\\") escaped = true;
      else if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') inString = true;
    else if (ch === "{" || ch === "[") depth += 1;
    else if (ch === "}" || ch === "]") depth -= 1;
  }
  return depth > 0 || inString;
}
