import { describe, expect, it } from "vitest";
import { generatePassphraseRoomId, generateRoomId, normalizeRoomId } from "./room.js";

describe("generateRoomId", () => {
  it("produces ids accepted by normalizeRoomId", () => {
    for (let i = 0; i < 20; i++) {
      const id = generateRoomId();
      expect(normalizeRoomId(id)).toBe(id);
      expect(id.length).toBe(12);
    }
  });

  it("respects length bounds", () => {
    expect(generateRoomId(4).length).toBe(4);
    expect(generateRoomId(64).length).toBe(64);
    expect(generateRoomId(2).length).toBe(4);
    expect(generateRoomId(100).length).toBe(64);
  });
});

describe("generatePassphraseRoomId", () => {
  it("produces ids accepted by normalizeRoomId", () => {
    for (let i = 0; i < 30; i++) {
      const id = generatePassphraseRoomId();
      expect(normalizeRoomId(id)).toBe(id);
      expect(id.length).toBeGreaterThanOrEqual(4);
      expect(id.length).toBeLessThanOrEqual(64);
    }
  });
});

describe("normalizeRoomId", () => {
  it("accepts valid ids", () => {
    expect(normalizeRoomId("  nursery-01_ ")).toBe("nursery-01_");
    expect(normalizeRoomId("abcd")).toBe("abcd");
  });

  it("rejects too short or invalid", () => {
    expect(normalizeRoomId("ab")).toBeUndefined();
    expect(normalizeRoomId("bad space")).toBeUndefined();
  });
});
