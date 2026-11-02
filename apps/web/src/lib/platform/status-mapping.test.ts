import { describe, expect, it } from "vitest";
import { causeHintOf, linkHintOf, qualityOf, withHint } from "./status-mapping";

describe("calm hints", () => {
  it("map the core's hint keys and ignore unknown ones", () => {
    expect(linkHintOf("clock_skew")).toBe("clock_skew");
    expect(linkHintOf("recovery_path_unavailable")).toBe("recovery_path_unavailable");
    expect(linkHintOf("something_new")).toBeNull();
    expect(linkHintOf("toString")).toBeNull();
  });

  it("are switched on and off without duplicates", () => {
    const on = withHint([], "clock_skew", true);
    expect(on).toEqual(["clock_skew"]);
    expect(withHint(on, "clock_skew", true)).toEqual(["clock_skew"]);
    expect(withHint(on, "recovery_path_unavailable", true)).toEqual([
      "clock_skew",
      "recovery_path_unavailable",
    ]);
    expect(withHint(on, "clock_skew", false)).toEqual([]);
    expect(withHint(on, "unknown_key", true)).toBe(on);
  });
});

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
    expect(causeHintOf("pair_again")).toBe("pair_again");
    expect(causeHintOf("baby_unrecognised")).toBe("baby_unrecognised");
    expect(causeHintOf("toString")).toBeNull();
    expect(causeHintOf(null)).toBeNull();
  });
});
