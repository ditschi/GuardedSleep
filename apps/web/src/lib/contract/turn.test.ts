import { describe, expect, it } from "vitest";
import { resolveTurnSave, validateTurnServer } from "./turn";

describe("validateTurnServer", () => {
  it.each([
    "turn:203.0.113.10:3478",
    "turns:turn.example.org:5349?transport=tcp",
    "turn:[2001:db8::1]:3478",
    "turn:host",
    "  turn:host  ",
  ])("accepts the address %j", (url) => {
    expect(validateTurnServer(url, "", "")).toBe("valid");
  });

  it.each([
    "",
    "stun:x",
    "turn:",
    "turn: host",
    "turn:host:0",
    "turn:host:70000",
    "turn:host?transport=sctp",
    "https://x",
  ])("rejects the address %j", (url) => {
    expect(validateTurnServer(url, "", "")).toBe("invalid_address");
  });

  it("accepts both credentials or neither", () => {
    expect(validateTurnServer("turn:h", "u", "pw")).toBe("valid");
    expect(validateTurnServer("turn:h", "", "")).toBe("valid");
  });

  it.each([
    ["user only", "u", ""],
    ["password only", "", "pw"],
    ["user name of 257 characters", "u".repeat(257), "pw"],
    ["password of 257 characters", "u", "p".repeat(257)],
  ])("rejects %s", (_name, user, pass) => {
    expect(validateTurnServer("turn:h", user, pass)).toBe("invalid_credentials");
  });

  it("allows 256 characters", () => {
    expect(validateTurnServer("turn:h", "u".repeat(256), "p".repeat(256))).toBe("valid");
  });
});

describe("resolveTurnSave", () => {
  const saved = { url: "turn:h", username: "u", password: "pw" };

  it("keeps the saved password for an unchanged address and user name", () => {
    expect(resolveTurnSave(saved, "turn:h", "u", "")).toEqual({ result: "saved", entry: saved });
  });

  it("does not keep it when the address or user name changed", () => {
    expect(resolveTurnSave(saved, "turn:other", "u", "").result).toBe("invalid_credentials");
    expect(resolveTurnSave(saved, "turn:h", "v", "").result).toBe("invalid_credentials");
  });

  it("clears on an empty address", () => {
    expect(resolveTurnSave(saved, " ", "", "")).toEqual({ result: "cleared", entry: null });
  });

  it("leaves the saved entry on an invalid save", () => {
    expect(resolveTurnSave(saved, "stun:x", "u", "pw")).toEqual({
      result: "invalid_address",
      entry: saved,
    });
  });
});
