import { describe, expect, it } from "vitest";
import { SignalingRoom, RoomRegistry } from "./room.js";

const sock = {} as import("ws").WebSocket;

describe("SignalingRoom", () => {
  it("allows one baby and multiple parents", () => {
    const room = new SignalingRoom("r1", 8);
    expect(room.add({ id: "b", role: "baby", socket: sock })).toBe("ok");
    expect(room.add({ id: "p1", role: "parent", socket: sock })).toBe("ok");
    expect(room.add({ id: "p2", role: "parent", socket: sock })).toBe("ok");
    expect(
      room
        .listMembers()
        .map((m) => m.clientId)
        .sort(),
    ).toEqual(["b", "p1", "p2"]);
  });

  it("rejects a second baby", () => {
    const room = new SignalingRoom("r1", 8);
    expect(room.add({ id: "b1", role: "baby", socket: sock })).toBe("ok");
    expect(room.add({ id: "b2", role: "baby", socket: sock })).toBe("baby_taken");
  });

  it("enforces parent cap", () => {
    const room = new SignalingRoom("r1", 2);
    expect(room.add({ id: "b", role: "baby", socket: sock })).toBe("ok");
    expect(room.add({ id: "p1", role: "parent", socket: sock })).toBe("ok");
    expect(room.add({ id: "p2", role: "parent", socket: sock })).toBe("ok");
    expect(room.add({ id: "p3", role: "parent", socket: sock })).toBe("parent_cap");
  });

  it("getPeer returns one other peer", () => {
    const room = new SignalingRoom("r1", 8);
    room.add({ id: "a", role: "baby", socket: sock });
    room.add({ id: "b", role: "parent", socket: sock });
    expect(room.getPeer("a")?.id).toBe("b");
    expect(room.getPeer("b")?.id).toBe("a");
  });
});

describe("RoomRegistry", () => {
  it("getOrCreate returns same room", () => {
    const reg = new RoomRegistry(8);
    const r1 = reg.getOrCreate("x");
    const r2 = reg.getOrCreate("x");
    expect(r1).toBe(r2);
  });

  it("deleteIfEmpty removes empty room", () => {
    const reg = new RoomRegistry(8);
    reg.getOrCreate("x");
    reg.deleteIfEmpty("x");
    const again = reg.getOrCreate("x");
    expect(again.size).toBe(0);
  });
});
