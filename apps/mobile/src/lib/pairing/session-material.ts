import { decodeBase64UrlToBytes } from "./compact-invite.js";

/** Session-only storage so the PSK is not persisted in localStorage. */
export const PAIR_ROOM_KEY = "guardedsleep:pair-room";
export const PAIR_PSK_KEY = "guardedsleep:pair-psk-b64";

export function storePairingMaterial(roomId: string, pskB64: string): void {
  if (typeof sessionStorage === "undefined") return;
  sessionStorage.setItem(PAIR_ROOM_KEY, roomId);
  sessionStorage.setItem(PAIR_PSK_KEY, pskB64);
}

/** Base64url PSK string for this room if session storage matches (for building invite QR). */
export function getStoredPairingPskB64ForRoom(roomId: string): string | null {
  if (typeof sessionStorage === "undefined") return null;
  const storedRoom = sessionStorage.getItem(PAIR_ROOM_KEY);
  const k = sessionStorage.getItem(PAIR_PSK_KEY);
  if (!k) return null;
  if (storedRoom != null && storedRoom !== roomId) return null;
  return k;
}

/** Returns PSK bytes if session storage matches this room (or no room was stored yet). */
export function takePairingPskForRoom(roomId: string): Uint8Array | null {
  if (typeof sessionStorage === "undefined") return null;
  const storedRoom = sessionStorage.getItem(PAIR_ROOM_KEY);
  const k = sessionStorage.getItem(PAIR_PSK_KEY);
  if (!k) return null;
  if (storedRoom != null && storedRoom !== roomId) return null;
  return decodeBase64UrlToBytes(k);
}
