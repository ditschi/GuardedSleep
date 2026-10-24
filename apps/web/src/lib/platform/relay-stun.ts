import type { TurnEntry } from "../contract/turn";
import { RELAY_STUN_PORT } from "./policy";

/**
 * ICE servers come from the configured relay, never from a public default: the relay answers
 * STUN Binding requests on UDP 3478 (add-rust-signaling-server). Only connection metadata reaches
 * a server we run or the user chose (AGENTS.md section 7.1). No sipgate, linphone or Google.
 */
export function relayStunUrls(signalingUrls: readonly string[]): string[] {
  const hosts = new Set<string>();
  for (const raw of signalingUrls) {
    try {
      const host = new URL(raw.trim()).hostname;
      if (host) hosts.add(host);
    } catch {
      // An unparsable entry is skipped; the signaling transport skips it as well.
    }
  }
  // `URL.hostname` keeps the brackets of an IPv6 literal.
  return [...hosts].map((host) => `stun:${host}:${RELAY_STUN_PORT}`);
}

export function relayIceServers(signalingUrls: readonly string[]): RTCIceServer[] {
  const urls = relayStunUrls(signalingUrls);
  return urls.length === 0 ? [] : [{ urls }];
}

/** The relay STUN entry first, then the user's TURN server with its credentials when set. */
export function iceServers(
  signalingUrls: readonly string[],
  turn: TurnEntry | null,
): RTCIceServer[] {
  const servers = relayIceServers(signalingUrls);
  if (turn === null) return servers;
  const entry: RTCIceServer = { urls: [turn.url] };
  if (turn.username !== "") entry.username = turn.username;
  if (turn.password !== "") entry.credential = turn.password;
  return [...servers, entry];
}

/**
 * Where a page without configured signaling URLs connects: the server that serves it (offline PC
 * mode, `--serve-web`), or the dev server's companion on 8787. Never a third party.
 */
export function defaultSignalingUrls(
  location: { protocol: string; hostname: string; host: string },
  dev: boolean,
): string[] {
  if (dev) return [`ws://${location.hostname}:8787`];
  return [`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}`];
}
