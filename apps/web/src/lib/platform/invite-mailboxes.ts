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
    else if (url.startsWith("ws://")) out.push({ kind: "lan", url });
  }
  return out;
}

/**
 * The signaling URLs a parent opens for a received invite. Nostr mailboxes are ignored until the
 * core speaks that protocol (task 3.5); relay and LAN mailboxes both use the relay protocol today.
 */
export function signalingUrlsOf(mailboxes: readonly InviteMailbox[]): string[] {
  return mailboxes.filter((m) => m.kind !== "nostr").map((m) => m.url);
}
