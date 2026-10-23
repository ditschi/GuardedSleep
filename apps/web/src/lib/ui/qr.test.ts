import { describe, expect, it } from "vitest";
import { QR_MARGIN, qrShape } from "./qr";

const invite = JSON.stringify({
  v: 3,
  k: "A".repeat(43),
  s: ["wss://relay.example.test"],
  x: 1_900_000_000,
});

describe("qrShape", () => {
  it("draws the code as a path inside a square with a quiet zone", () => {
    const shape = qrShape(invite);
    // Version 1 is 21 modules; an invite of this size needs a bigger one.
    expect(shape.size).toBeGreaterThan(21 + 2 * QR_MARGIN);
    expect(shape.path.startsWith("M")).toBe(true);
    expect(shape.path).toMatch(/^(?:M\d+ \d+h\d+v1h-\d+z)+$/);
  });

  it("keeps every module inside the quiet zone", () => {
    const { size, path } = qrShape(invite);
    for (const run of path.matchAll(/M(\d+) (\d+)h(\d+)/g)) {
      const [x, y, width] = [Number(run[1]), Number(run[2]), Number(run[3])];
      expect(x).toBeGreaterThanOrEqual(QR_MARGIN);
      expect(y).toBeGreaterThanOrEqual(QR_MARGIN);
      expect(x + width).toBeLessThanOrEqual(size - QR_MARGIN);
    }
  });

  it("is deterministic and differs per text", () => {
    expect(qrShape(invite)).toEqual(qrShape(invite));
    expect(qrShape(invite + " ").path).not.toBe(qrShape(invite).path);
  });
});
