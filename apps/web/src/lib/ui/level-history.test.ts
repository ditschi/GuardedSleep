import { describe, expect, it } from "vitest";
import { LevelHistory } from "./level-history";

describe("LevelHistory", () => {
  it("is flat while empty", () => {
    const h = new LevelHistory();
    const bars = h.bars(120_000);
    expect(bars).toHaveLength(60);
    expect(bars.every((b) => b === 0)).toBe(true);
  });

  it("puts a sample into its bucket, oldest first", () => {
    const h = new LevelHistory();
    h.push(60_000, 0.5);
    h.push(61_000, 0.4);
    h.push(120_000, 0.9);
    const bars = h.bars(120_000);
    expect(bars[0]).toBe(0.5);
    expect(bars[1]).toBe(0.4);
    expect(bars[59]).toBe(0.9);
    expect(bars[2]).toBe(0);
  });

  it("keeps the maximum of one bucket", () => {
    const h = new LevelHistory();
    h.push(100_000, 0.2);
    h.push(100_400, 0.7);
    h.push(100_800, 0.3);
    expect(h.bars(120_000)[40]).toBe(0.7);
  });

  it("ignores samples older than the window and drops very old ones", () => {
    const h = new LevelHistory();
    h.push(59_999, 1);
    expect(h.bars(120_000).every((b) => b === 0)).toBe(true);
    h.push(300_000, 0.5);
    h.push(310_000, 0.5);
    expect(h.bars(310_000).filter((b) => b > 0)).toHaveLength(2);
  });

  it("clamps levels and clears", () => {
    const h = new LevelHistory();
    h.push(110_000, 4);
    h.push(111_000, Number.NaN);
    const bars = h.bars(120_000);
    expect(bars[50]).toBe(1);
    expect(bars[51]).toBe(0);
    h.clear();
    expect(h.bars(120_000).every((b) => b === 0)).toBe(true);
  });
});
