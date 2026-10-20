import { describe, expect, it } from "vitest";
import { RateLimiter } from "./rate-limit.js";

describe("RateLimiter", () => {
  it("allows a burst up to the limit then refills over time", () => {
    let t = 0;
    const rl = new RateLimiter(3, () => t);
    expect([rl.take("a"), rl.take("a"), rl.take("a"), rl.take("a")]).toEqual([
      true,
      true,
      true,
      false,
    ]);
    expect(rl.take("b")).toBe(true);
    t += 20_000;
    expect(rl.take("a")).toBe(true);
    expect(rl.take("a")).toBe(false);
  });

  it("is disabled at 0", () => {
    const rl = new RateLimiter(0);
    for (let i = 0; i < 100; i++) expect(rl.take("a")).toBe(true);
  });

  it("prunes refilled buckets", () => {
    let t = 0;
    const rl = new RateLimiter(2, () => t);
    rl.take("a");
    t += 60_000;
    rl.prune();
    expect(rl.take("a")).toBe(true);
  });
});
