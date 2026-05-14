import { afterEach, describe, expect, it } from "vitest";
import {
  listConnections,
  removeConnection,
  setFallbackSignalingUrls,
  upsertConnection,
} from "./connection-store.js";

const KEY = "guardedsleep:saved-connections";

afterEach(() => {
  localStorage.clear();
});

describe("connection-store", () => {
  it("upserts and dedupes by room + signalingUrl", () => {
    const a = upsertConnection({ roomId: "room1", signalingUrl: "ws://a" });
    expect(listConnections()).toHaveLength(1);
    const b = upsertConnection({ roomId: "room1", signalingUrl: "ws://a" });
    expect(b.id).toBe(a.id);
    expect(listConnections()).toHaveLength(1);
    expect(listConnections()[0]!.lastUsedAt).toBeGreaterThanOrEqual(a.lastUsedAt);
  });

  it("keeps distinct servers separate", () => {
    upsertConnection({ roomId: "room1", signalingUrl: "ws://a" });
    upsertConnection({ roomId: "room1", signalingUrl: "ws://b" });
    expect(listConnections()).toHaveLength(2);
  });

  it("removeConnection drops entry", () => {
    const row = upsertConnection({ roomId: "r", signalingUrl: "ws://x" });
    removeConnection(row.id);
    expect(listConnections()).toHaveLength(0);
    expect(localStorage.getItem(KEY)).toBe("[]");
  });

  it("persists and sanitizes fallbackSignalingUrls", () => {
    const row = upsertConnection({
      roomId: "room1",
      signalingUrl: "ws://primary",
      fallbackSignalingUrls: ["ws://lan:8787", "wss://nas/", "not-a-url", "   "],
    });
    expect(row.fallbackSignalingUrls).toEqual(["ws://lan:8787", "wss://nas/"]);
    const reloaded = listConnections()[0]!;
    expect(reloaded.fallbackSignalingUrls).toEqual(["ws://lan:8787", "wss://nas/"]);
  });

  it("setFallbackSignalingUrls overwrites and can clear", () => {
    const row = upsertConnection({ roomId: "r", signalingUrl: "ws://x" });
    setFallbackSignalingUrls(row.id, ["ws://lan:8787"]);
    expect(listConnections()[0]!.fallbackSignalingUrls).toEqual(["ws://lan:8787"]);
    setFallbackSignalingUrls(row.id, []);
    expect(listConnections()[0]!.fallbackSignalingUrls).toBeUndefined();
  });
});
