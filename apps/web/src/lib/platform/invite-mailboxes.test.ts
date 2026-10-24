import { describe, expect, it } from "vitest";
import { mailboxesForUrls, signalingUrlsOf } from "./invite-mailboxes";

describe("invite mailboxes", () => {
  it("maps wss to an own relay and ws to a LAN mailbox, skipping other schemes", () => {
    expect(
      mailboxesForUrls(["wss://gs.example.com/sig", "ws://192.168.1.5:8787", "http://x", ""]),
    ).toEqual([
      { kind: "relay", url: "wss://gs.example.com/sig" },
      { kind: "lan", url: "ws://192.168.1.5:8787" },
    ]);
  });

  it("opens relay and LAN mailboxes only, until the core speaks Nostr", () => {
    expect(
      signalingUrlsOf([
        { kind: "nostr", url: "wss://relay.example.org" },
        { kind: "relay", url: "wss://gs.example.com" },
        { kind: "lan", url: "ws://10.0.2.2:8787" },
      ]),
    ).toEqual(["wss://gs.example.com", "ws://10.0.2.2:8787"]);
  });
});
