import { loadCoreNode } from "@guardedsleep/core-wasm/node";
import { describe, expect, it } from "vitest";
import { PUBLIC_NOSTR_RELAYS } from "../public-lists/generated/public-lists";
import { nostrRelayUrls, relayCandidates } from "./public-relays";

const psk = Uint8Array.from({ length: 32 }, (_, i) => i + 1);

describe("the curated list", () => {
  it("is the shipped data: at least 6 relays, wss, one per operator", () => {
    expect(PUBLIC_NOSTR_RELAYS.length).toBeGreaterThanOrEqual(6);
    expect(new Set(PUBLIC_NOSTR_RELAYS.map((r) => r.operator)).size).toBe(
      PUBLIC_NOSTR_RELAYS.length,
    );
    for (const r of PUBLIC_NOSTR_RELAYS) expect(r.url).toBe(`wss://${r.host}`);
  });

  it("maps to core candidates without losing the operator and hoster", () => {
    const first = PUBLIC_NOSTR_RELAYS[0];
    expect(relayCandidates()[0]).toEqual({
      host: first?.host,
      operator: first?.operator,
      hoster: first?.hoster,
    });
  });
});

describe("nostrRelayUrls", () => {
  const core = loadCoreNode();

  it("is empty while the release gate is off", () => {
    expect(nostrRelayUrls({ core, psk, devRelays: null, publicEnabled: false })).toEqual([]);
  });

  it("picks three relays of distinct operators from the list when the gate is on", () => {
    const urls = nostrRelayUrls({ core, psk, devRelays: null, publicEnabled: true });
    expect(urls).toHaveLength(3);
    const operators = urls.map(
      (u) => PUBLIC_NOSTR_RELAYS.find((r) => r.url === u)?.operator ?? "unlisted",
    );
    expect(operators).not.toContain("unlisted");
    expect(new Set(operators).size).toBe(3);
    expect(nostrRelayUrls({ core, psk, devRelays: null, publicEnabled: true })).toEqual(urls);
  });

  it("lets a set dev list, even an empty one, replace the public relays", () => {
    expect(nostrRelayUrls({ core, psk, devRelays: [], publicEnabled: true })).toEqual([]);
    expect(
      nostrRelayUrls({ core, psk, devRelays: ["ws://127.0.0.1:9"], publicEnabled: true }),
    ).toEqual(["ws://127.0.0.1:9"]);
  });

  it("spreads over the list for different keys", () => {
    const seen = new Set<string>();
    for (let n = 0; n < 12; n++) {
      const key = Uint8Array.from(psk, (b) => (b + n * 17) % 256);
      for (const u of nostrRelayUrls({ core, psk: key, devRelays: null, publicEnabled: true })) {
        seen.add(u);
      }
    }
    expect(seen.size).toBeGreaterThan(3);
  });
});
