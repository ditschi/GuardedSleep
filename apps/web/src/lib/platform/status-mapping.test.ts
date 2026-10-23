import { describe, expect, it } from "vitest";
import { causeHintOf, qualityOf } from "./status-mapping";

describe("qualityOf", () => {
  const cases: Array<[number, number, string | null]> = [
    [10, 5, null], // fewer than 20 packets: no judgement
    [1000, 0, "good"],
    [980, 20, "good"], // 2 %
    [970, 30, "medium"], // 3 %
    [910, 90, "medium"], // 9 %
    [900, 100, "poor"], // 10 %
    [100, -4, "good"], // a negative loss counter is clamped
  ];
  it.each(cases)("%i received, %i lost -> %s", (received, lost, expected) => {
    expect(qualityOf(received, lost)).toBe(expected);
  });
});

describe("causeHintOf", () => {
  it("maps the core's cause keys and nothing else", () => {
    expect(causeHintOf("server_unreachable")).toBe("server_unreachable");
    expect(causeHintOf("peer_absent")).toBe("peer_absent");
    expect(causeHintOf("media_blocked")).toBe("media_blocked");
    expect(causeHintOf("update_required")).toBeNull();
    expect(causeHintOf("toString")).toBeNull();
    expect(causeHintOf(null)).toBeNull();
  });
});
