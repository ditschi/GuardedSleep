import { parseCompactInviteJson } from "$lib/pairing/compact-invite.js";
import { normalizeRoomId } from "$lib/utils/room.js";
import { validateSignalingUrl } from "$lib/signaling-url.js";

export type ParsedInvite = {
  room: string;
  signal: string;
  /** From compact QR JSON — enables DataChannel `pair_proof` (PSK + DTLS fingerprints). */
  pskB64?: string;
};

/**
 * Invite deep link: `/pair?room=…&signal=…` (no `role` in QR so baby and parent can share one code).
 */
export function buildInviteUrl(opts: { origin: string; room: string; signal: string }): string {
  const u = new URL("/pair", opts.origin.replace(/\/$/, ""));
  u.searchParams.set("room", opts.room.trim());
  u.searchParams.set("signal", opts.signal.trim());
  return u.toString();
}

export function parseInviteSearchParams(params: URLSearchParams): Partial<ParsedInvite> {
  const roomRaw = params.get("room")?.trim() ?? "";
  const signalRaw = params.get("signal")?.trim() ?? "";
  const room = normalizeRoomId(roomRaw);
  const signalErr = signalRaw === "" ? "empty" : validateSignalingUrl(signalRaw);
  const signal = signalErr === null ? signalRaw : undefined;
  const out: Partial<ParsedInvite> = {};
  if (room) out.room = room;
  if (signal) out.signal = signal;
  return out;
}

/** Parse compact JSON invite, https invite URL, or legacy path. */
export function parseInviteFromString(text: string): Partial<ParsedInvite> {
  const t = text.trim();
  if (!t) return {};
  if (t.startsWith("{")) {
    const c = parseCompactInviteJson(t);
    if (c) return { room: c.r, signal: c.s, pskB64: c.k };
    return {};
  }
  try {
    const u = new URL(t);
    return parseInviteSearchParams(u.searchParams);
  } catch {
    return {};
  }
}

export function isCompleteInvite(p: Partial<ParsedInvite>): p is ParsedInvite {
  return typeof p.room === "string" && typeof p.signal === "string";
}
