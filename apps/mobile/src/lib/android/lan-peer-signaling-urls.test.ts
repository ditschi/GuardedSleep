import { describe, expect, it } from "vitest";
import { lanPrependSignalingUrls } from "./lan-peer-signaling-urls.js";

describe("lanPrependSignalingUrls", () => {
  it("includes opaque-token peers regardless of room", () => {
    const urls = lanPrependSignalingUrls(
      { a: { room: "", host: "10.0.0.5", port: 44000, token: "abc" } },
      "room-x",
    );
    expect(urls).toEqual(["ws://10.0.0.5:44000"]);
  });

  it("includes legacy room fragment only when it matches", () => {
    expect(
      lanPrependSignalingUrls(
        { a: { room: "other", host: "10.0.0.5", port: 1, token: "" } },
        "mine",
      ),
    ).toEqual([]);
    expect(
      lanPrependSignalingUrls(
        { a: { room: "mine", host: "10.0.0.5", port: 1, token: "" } },
        "mine",
      ),
    ).toEqual(["ws://10.0.0.5:1"]);
  });

  it("dedupes same host:port and skips invalid ports", () => {
    const urls = lanPrependSignalingUrls(
      {
        a: { room: "", host: "192.168.1.10", port: 9000, token: "t1" },
        b: { room: "", host: "192.168.1.10", port: 9000, token: "t2" },
        c: { room: "x", host: "192.168.1.11", port: 0, token: "bad" },
      },
      "any-room",
    );
    expect(urls).toEqual(["ws://192.168.1.10:9000"]);
  });

  it("treats empty token as legacy room match only", () => {
    expect(
      lanPrependSignalingUrls({ a: { room: "r1", host: "10.0.0.1", port: 2, token: "" } }, ""),
    ).toEqual([]);
  });
});
