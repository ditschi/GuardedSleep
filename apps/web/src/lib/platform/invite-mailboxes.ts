import type { InviteMailbox } from "@guardedsleep/core-wasm";

/**
 * The configured signaling URLs as invite mailboxes. Every configured address speaks the relay
 * protocol (the Nostr relays are named separately, see {@link nostrMailboxesForUrls}):
 * `wss://` is an own relay (`r:`), `ws://` can only be a LAN address (`l:`, the core accepts
 * RFC 1918 and ULA literals and, in dev builds, loopback). Other schemes are skipped, so the core
 * reports an empty invite as not valid.
 */
export function mailboxesForUrls(urls: readonly string[]): InviteMailbox[] {
  const out: InviteMailbox[] = [];
  for (const url of urls) {
    if (url.startsWith("wss://")) out.push({ kind: "relay", url });
    else {
      const lan = lanMailboxOf(url);
      if (lan !== null) out.push(lan);
    }
  }
  return out;
}

/**
 * A `ws://` URL with a private or loopback literal host as an `l:host:port` mailbox: the path is
 * dropped (a LAN mailbox is `ip:port`; the server accepts the upgrade on any path) and a missing
 * port is 80. Any other `ws://` URL is not a LAN address and is skipped.
 */
function lanMailboxOf(url: string): InviteMailbox | null {
  if (!url.startsWith("ws://")) return null;
  let parsed: URL;
  try {
    parsed = new URL(url);
  } catch {
    return null;
  }
  if (!isPrivateOrLoopbackLiteral(parsed.hostname)) return null;
  const port = parsed.port === "" ? "80" : parsed.port;
  return { kind: "lan", url: `ws://${parsed.hostname}:${port}` };
}

function isPrivateOrLoopbackLiteral(host: string): boolean {
  if (host === "localhost" || host === "[::1]") return true;
  const v4 = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(host);
  if (v4) {
    const [a = 0, b = 0] = v4.slice(1).map(Number);
    return a === 127 || a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168);
  }
  // IPv6 unique local addresses (fc00::/7).
  return /^\[f[cd][0-9a-f]{2}:/i.test(host);
}

/**
 * The signaling URLs a parent opens for a received invite: relay and LAN mailboxes both use the
 * relay protocol and stay in the platform's address list. The Nostr mailboxes are not in it; the
 * core opens them itself (see {@link nostrMailboxesOf}).
 */
export function signalingUrlsOf(mailboxes: readonly InviteMailbox[]): string[] {
  return mailboxes.filter((m) => m.kind !== "nostr").map((m) => m.url);
}

/** The public Nostr relays of an invite or a mailbox update, for the core's `mailboxes` setting. */
export function nostrMailboxesOf(mailboxes: readonly InviteMailbox[]): InviteMailbox[] {
  return mailboxes.filter((m) => m.kind === "nostr");
}

/**
 * Nostr relays as invite mailboxes. `wss://host` is a public relay; `ws://` is accepted only for a
 * loopback relay of a development or E2E build (the core's invite codec decides, this only maps).
 */
export function nostrMailboxesForUrls(urls: readonly string[]): InviteMailbox[] {
  return urls
    .filter((url) => url.startsWith("wss://") || url.startsWith("ws://"))
    .map((url) => ({ kind: "nostr" as const, url }));
}
