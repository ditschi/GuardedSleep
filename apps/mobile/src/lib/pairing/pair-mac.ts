import { PROTOCOL_VERSION } from "@guardedsleep/protocol";
import { extractSha256FingerprintFromSdp } from "./dtls-fingerprint.js";

async function hmacSha256Hex(key: Uint8Array, message: string): Promise<string> {
  const enc = new TextEncoder();
  const ck = await crypto.subtle.importKey(
    "raw",
    key as unknown as BufferSource,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign("HMAC", ck, enc.encode(message));
  const bytes = new Uint8Array(sig);
  let hex = "";
  for (let i = 0; i < bytes.length; i++) {
    hex += bytes[i]!.toString(16).padStart(2, "0");
  }
  return hex;
}

/**
 * Both peers derive the same MAC from the QR PSK and the two DTLS certificate fingerprints (sorted).
 */
export async function computePairProofMac(
  psk: Uint8Array,
  roomId: string,
  localSdp: string,
  remoteSdp: string,
): Promise<string | null> {
  const a = extractSha256FingerprintFromSdp(localSdp);
  const b = extractSha256FingerprintFromSdp(remoteSdp);
  if (!a || !b) return null;
  const [x, y] = a < b ? [a, b] : [b, a];
  const canonical = `${roomId}|${x}|${y}|${PROTOCOL_VERSION}`;
  return hmacSha256Hex(psk, canonical);
}
