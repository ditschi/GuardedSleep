import { describe, expect, it } from "vitest";
import {
  mailboxesForUrls,
  nostrMailboxesForUrls,
  nostrMailboxesOf,
  signalingUrlsOf,
} from "./invite-mailboxes";

describe("invite mailboxes", () => {
  it("maps wss to an own relay and ws to a LAN mailbox, skipping other schemes", () => {
    expect(
      mailboxesForUrls(["wss://gs.example.com/sig", "ws://192.168.1.5:8787", "http://x", ""]),
    ).toEqual([
      { kind: "relay", url: "wss://gs.example.com/sig" },
      { kind: "lan", url: "ws://192.168.1.5:8787" },
    ]);
  });

  it("maps a private or loopback ws URL to ip:port, dropping the path", () => {
    expect(
      mailboxesForUrls([
        "ws://127.0.0.1:41234/signaling",
        "ws://192.168.1.5/signaling",
        "ws://10.0.2.2:8787",
        "ws://172.20.0.3:9/x",
        "ws://localhost:8787/s",
        "ws://[::1]:8787/signaling",
        "ws://[fd12:3456::1]:8787",
      ]),
    ).toEqual([
      { kind: "lan", url: "ws://127.0.0.1:41234" },
      { kind: "lan", url: "ws://192.168.1.5:80" },
      { kind: "lan", url: "ws://10.0.2.2:8787" },
      { kind: "lan", url: "ws://172.20.0.3:9" },
      { kind: "lan", url: "ws://localhost:8787" },
      { kind: "lan", url: "ws://[::1]:8787" },
      { kind: "lan", url: "ws://[fd12:3456::1]:8787" },
    ]);
  });

  it("skips a ws URL that is not a private literal, and keeps the path of a wss relay", () => {
    expect(
      mailboxesForUrls(["ws://8.8.8.8:1/x", "ws://172.32.0.1:1", "ws://pc.local:8787", "ws://"]),
    ).toEqual([]);
    expect(mailboxesForUrls(["wss://gs.example.com/sig"])).toEqual([
      { kind: "relay", url: "wss://gs.example.com/sig" },
    ]);
  });

  it("opens relay and LAN mailboxes as signaling URLs and leaves the Nostr relays to the core", () => {
    const set = [
      { kind: "nostr" as const, url: "wss://relay.example.org" },
      { kind: "relay" as const, url: "wss://gs.example.com" },
      { kind: "lan" as const, url: "ws://10.0.2.2:8787" },
    ];
    expect(signalingUrlsOf(set)).toEqual(["wss://gs.example.com", "ws://10.0.2.2:8787"]);
    expect(nostrMailboxesOf(set)).toEqual([{ kind: "nostr", url: "wss://relay.example.org" }]);
  });

  it("names Nostr relays by their URL and skips other schemes", () => {
    expect(
      nostrMailboxesForUrls(["wss://relay-a.example", "ws://127.0.0.1:7777", "http://x", ""]),
    ).toEqual([
      { kind: "nostr", url: "wss://relay-a.example" },
      { kind: "nostr", url: "ws://127.0.0.1:7777" },
    ]);
  });
});
