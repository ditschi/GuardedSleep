import { describe, expect, it } from "vitest";
import { defaultSignalingUrls, relayIceServers, relayStunUrls } from "./relay-stun";

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

  it("has no public STUN default: without a relay there is no ICE server", () => {
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
