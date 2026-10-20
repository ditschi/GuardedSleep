import { describe, expect, it } from "vitest";
import { devSignalingConnectHint } from "./dev-connect-hint.js";

describe("devSignalingConnectHint", () => {
  it("flags loopback URLs", () => {
    expect(devSignalingConnectHint("ws://127.0.0.1:8787")).toEqual({ kind: "loopback" });
    expect(devSignalingConnectHint("wss://localhost:8443/signaling")).toEqual({ kind: "loopback" });
  });

  it("suggests plain ws on 8787 for LAN WSS (Docker / self-signed)", () => {
    expect(devSignalingConnectHint("wss://192.168.1.42:8443/signaling")).toEqual({
      kind: "wss_lan",
      suggestedPlainWs: "ws://192.168.1.42:8787",
    });
  });

  it("leaves plain LAN ws unchanged", () => {
    expect(devSignalingConnectHint("ws://192.168.1.42:8787")).toEqual({ kind: "none" });
  });
});
