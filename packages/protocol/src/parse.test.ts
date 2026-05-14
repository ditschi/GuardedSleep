import { describe, expect, it } from "vitest";
import { PROTOCOL_VERSION } from "./messages.js";
import {
  assertCompatibleProtocol,
  parseClientMessage,
  parseServerMessage,
  stringifyClientMessage,
} from "./parse.js";
import {
  encodeListen,
  encodeNoiseAlert,
  encodeOpenSettings,
  encodePairProof,
  parseListenCommand,
  parseNoiseAlert,
  parseOpenSettings,
  parsePairProof,
} from "./datachannel.js";
import { encodeHeartbeat, parseHeartbeat } from "./heartbeat.js";

describe("parseClientMessage", () => {
  it("parses signal with optional to", () => {
    const raw = stringifyClientMessage({
      type: "signal",
      roomId: "r1",
      to: "peer-uuid",
      payload: { x: 1 },
    });
    expect(parseClientMessage(raw)).toEqual({
      type: "signal",
      roomId: "r1",
      to: "peer-uuid",
      payload: { x: 1 },
    });
  });

  it("parses hello", () => {
    const raw = stringifyClientMessage({
      type: "hello",
      role: "baby",
      roomId: "room-1",
      protocolVersion: PROTOCOL_VERSION,
    });
    expect(parseClientMessage(raw)).toEqual({
      type: "hello",
      role: "baby",
      roomId: "room-1",
      protocolVersion: PROTOCOL_VERSION,
    });
  });

  it("rejects invalid room", () => {
    expect(
      parseClientMessage(
        JSON.stringify({ type: "hello", role: "baby", roomId: "", protocolVersion: 1 }),
      ),
    ).toBeUndefined();
  });

  it("parses ping", () => {
    expect(parseClientMessage(JSON.stringify({ type: "ping" }))).toEqual({ type: "ping" });
  });
});

describe("parseServerMessage", () => {
  it("parses error", () => {
    const msg = parseServerMessage(
      JSON.stringify({ type: "error", code: "ROOM_FULL", message: "full" }),
    );
    expect(msg).toEqual({ type: "error", code: "ROOM_FULL", message: "full" });
  });

  it("parses members", () => {
    const msg = parseServerMessage(
      JSON.stringify({
        type: "members",
        peers: [
          { clientId: "a", role: "baby" },
          { clientId: "b", role: "parent" },
        ],
      }),
    );
    expect(msg).toEqual({
      type: "members",
      peers: [
        { clientId: "a", role: "baby" },
        { clientId: "b", role: "parent" },
      ],
    });
  });
});

describe("protocol version", () => {
  it("assertCompatibleProtocol", () => {
    expect(assertCompatibleProtocol(PROTOCOL_VERSION)).toBe(true);
    expect(assertCompatibleProtocol(999)).toBe(false);
  });
});

describe("heartbeat", () => {
  it("roundtrips", () => {
    const raw = encodeHeartbeat(3);
    expect(parseHeartbeat(raw)).toMatchObject({ seq: 3, v: PROTOCOL_VERSION });
  });

  it("roundtrips battery and rms", () => {
    const raw = encodeHeartbeat(1, { battery: 0.82, rms: 0.03 });
    expect(parseHeartbeat(raw)).toMatchObject({
      seq: 1,
      v: PROTOCOL_VERSION,
      battery: 0.82,
      rms: 0.03,
    });
  });
});

describe("datachannel", () => {
  it("listen roundtrips", () => {
    expect(parseListenCommand(encodeListen(true))).toBe(true);
    expect(parseListenCommand(encodeListen(false))).toBe(false);
  });

  it("noise_alert roundtrips", () => {
    const raw = encodeNoiseAlert(0.06, 999);
    expect(parseNoiseAlert(raw)).toEqual({ rms: 0.06, t: 999 });
  });

  it("pair_proof roundtrips", () => {
    const mac = "Ab".repeat(32);
    expect(parsePairProof(encodePairProof(mac))).toBe(mac.toLowerCase());
  });

  it("open_settings parses", () => {
    expect(parseOpenSettings(encodeOpenSettings())).toBe(true);
    expect(parseOpenSettings("{}")).toBe(false);
  });
});
