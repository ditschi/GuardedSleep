import { describe, expect, it } from "vitest";
import { PUBLIC_STUN_SERVERS } from "../public-lists/generated/public-lists";
import { defaultSignalingUrls, iceServersFor, relayIceServers, relayStunUrls } from "./relay-stun";

describe("relay STUN", () => {
  it("derives one STUN URL per distinct relay host and nothing else", () => {
    expect(
      relayStunUrls(["wss://relay.example/sig", "ws://relay.example:8787", "ws://10.0.2.2:8787"]),
    ).toEqual(["stun:relay.example:3478", "stun:10.0.2.2:3478"]);
  });

  it("keeps IPv6 literals bracketed and skips unparsable entries", () => {
    expect(relayStunUrls(["ws://[fd00::1]:8787", "not a url", ""])).toEqual([
      "stun:[fd00::1]:3478",
    ]);
  });

  it("derives nothing from an empty relay list (the public default is iceServersFor)", () => {
    expect(relayIceServers([])).toEqual([]);
    const text = JSON.stringify(relayIceServers(["ws://x:1"]));
    expect(text).not.toMatch(/google|sipgate|linphone/);
    expect(relayIceServers(["ws://x:1"])).toEqual([{ urls: ["stun:x:3478"] }]);
  });

  it("defaults the signaling URL to the serving host, or the dev companion", () => {
    const here = { protocol: "https:", hostname: "gs.local", host: "gs.local:8443" };
    expect(defaultSignalingUrls(here, false)).toEqual(["wss://gs.local:8443/signaling"]);
    expect(defaultSignalingUrls({ ...here, protocol: "http:" }, false)).toEqual([
      "ws://gs.local:8443/signaling",
    ]);
    expect(defaultSignalingUrls(here, true)).toEqual(["ws://gs.local:8787"]);
  });
});

describe("iceServersFor", () => {
  const listed = ["stun:stun.one.test:443", "stun:stun.one.test:3478", "stun:stun.two.test:3478"];

  it("uses only the own relay's STUN when one is configured", () => {
    expect(iceServersFor(["wss://relay.example/sig"], { defaultStun: listed })).toEqual([
      { urls: ["stun:relay.example:3478"] },
    ]);
  });

  it("falls back to the curated default without an own relay", () => {
    expect(iceServersFor([], { defaultStun: listed })).toEqual([{ urls: listed }]);
  });

  it("asks no public STUN at all with 'only same Wi-Fi or my own server'", () => {
    expect(iceServersFor([], { onlyLocal: true, defaultStun: listed })).toEqual([]);
  });

  it("the generated default is two operators of data/stun-servers.json and nothing else", () => {
    const urls = iceServersFor([]).flatMap((s) => (Array.isArray(s.urls) ? s.urls : [s.urls]));
    const operators = new Set(
      urls.map((u) => PUBLIC_STUN_SERVERS.find((s) => s.url === u)?.operator ?? "unlisted"),
    );
    expect(operators.has("unlisted")).toBe(false);
    expect(operators.size).toBe(2);
    expect(JSON.stringify(urls)).not.toMatch(/google/);
  });
});
