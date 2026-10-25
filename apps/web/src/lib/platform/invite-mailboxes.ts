import type { InviteMailbox } from "@guardedsleep/core-wasm";

/**
 * The configured signaling URLs as invite mailboxes. Until the core opens mailboxes itself
 * (`alpha-connection-model` tasks 3.5/3.6) every configured address speaks the relay protocol:
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
 * The signaling URLs a parent opens for a received invite. Nostr mailboxes are ignored until the
 * core speaks that protocol (task 3.5); relay and LAN mailboxes both use the relay protocol today.
 */
export function signalingUrlsOf(mailboxes: readonly InviteMailbox[]): string[] {
  return mailboxes.filter((m) => m.kind !== "nostr").map((m) => m.url);
}
