import type { TurnEntry } from "../contract/turn";
import { DEFAULT_STUN_URLS } from "../public-lists/generated/public-lists";
import { RELAY_STUN_PORT } from "./policy";

/**
 * A configured relay brings its own STUN: it answers Binding requests on UDP 3478
 * (add-rust-signaling-server), so connection metadata reaches only a server we run or the user
 * chose (AGENTS.md section 7.1). Without one, {@link iceServersFor} falls back to the curated list.
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

/**
 * The ICE servers of an attempt (design.md section 7): with an own relay configured only its STUN;
 * otherwise the default pair of operators from `data/stun-servers.json`, unless "Only same Wi-Fi or
 * my own server" is on, which allows no public STUN at all. No host outside that list is ever used
 * (`just lint` enforces it). The user's own TURN server, when set, comes last with its credentials.
 */
export function iceServersFor(
  signalingUrls: readonly string[],
  options: { onlyLocal?: boolean; defaultStun?: readonly string[]; turn?: TurnEntry | null } = {},
): RTCIceServer[] {
  return [...stunServersFor(signalingUrls, options), ...turnServers(options.turn ?? null)];
}

function stunServersFor(
  signalingUrls: readonly string[],
  options: { onlyLocal?: boolean; defaultStun?: readonly string[] },
): RTCIceServer[] {
  const own = relayIceServers(signalingUrls);
  if (own.length > 0) return own;
  if (options.onlyLocal) return [];
  const urls = options.defaultStun ?? DEFAULT_STUN_URLS;
  return urls.length === 0 ? [] : [{ urls: [...urls] }];
}

function turnServers(turn: TurnEntry | null): RTCIceServer[] {
  if (turn === null) return [];
  const entry: RTCIceServer = { urls: [turn.url] };
  if (turn.username !== "") entry.username = turn.username;
  if (turn.password !== "") entry.credential = turn.password;
  return [entry];
}

/**
 * Where a page without configured signaling URLs connects: `/signaling` on the server that serves it
 * (`--serve-web`, the web image, a reverse proxy), or the dev server's companion on 8787. Never a
 * third party.
 */
export function defaultSignalingUrls(
  location: { protocol: string; hostname: string; host: string },
  dev: boolean,
): string[] {
  if (dev) return [`ws://${location.hostname}:8787`];
  // `/signaling` matches the one-domain reverse-proxy layout (docker/public-server/README.md);
  // the server and the bundled nginx image accept the upgrade on any path.
  return [`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/signaling`];
}
