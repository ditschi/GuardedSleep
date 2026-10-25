import type { CoreApi, RelayCandidate } from "@guardedsleep/core-wasm";
import { PUBLIC_NOSTR_RELAYS, type PublicRelay } from "../public-lists/generated/public-lists";

/** The shipped list in the shape the core's relay pick takes (the core carries no relay data). */
export function relayCandidates(
  list: readonly PublicRelay[] = PUBLIC_NOSTR_RELAYS,
): RelayCandidate[] {
  return list.map(({ host, operator, hoster }) => ({ host, operator, hoster }));
}

export interface NostrRelayChoice {
  core: Pick<CoreApi, "pickRelays">;
  psk: Uint8Array;
  /**
   * A local test relay list of the dev and E2E builds, `null` when none was set. A set list (even
   * an empty one) is the whole answer: tests that name their relays never reach a public one.
   */
  devRelays: string[] | null;
  /** The release gate, see `publicRelaysEnabled`. */
  publicEnabled: boolean;
  list?: readonly PublicRelay[];
}

/**
 * The Nostr relay URLs the baby adds to its invite: the dev test relays when set, otherwise (and
 * only when the release gate allows it) the core's deterministic pick from the curated list.
 */
export function nostrRelayUrls(choice: NostrRelayChoice): string[] {
  if (choice.devRelays !== null) return choice.devRelays;
  if (!choice.publicEnabled) return [];
  return choice.core.pickRelays(choice.psk, relayCandidates(choice.list)).map((m) => m.url);
}
