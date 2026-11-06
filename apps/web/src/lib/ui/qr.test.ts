import { loadCoreNode } from "@guardedsleep/core-wasm/node";
import {
  BinaryBitmap,
  HybridBinarizer,
  InvertedLuminanceSource,
  QRCodeReader,
  RGBLuminanceSource,
} from "@zxing/library";
import { describe, expect, it } from "vitest";
import { QR_MARGIN, qrShape, type QrShape } from "./qr";

const core = loadCoreNode();
const psk = Uint8Array.from({ length: 32 }, (_, i) => i + 7);

/** A fully used invite: a relay, three further relays and a LAN mailbox, close to the 300 byte limit. */
function maxInvite(): string {
  const built = core.buildInvite(
    psk,
    [
      { kind: "relay", url: "wss://signaling.gs-operator.example.org:8443/gs/signal" },
      { kind: "relay", url: "wss://relay-one.operator-a.example.eu" },
      { kind: "relay", url: "wss://relay-two.operator-b.example.ch" },
      { kind: "relay", url: "wss://relay-three.operator-c.example.de" },
      { kind: "lan", url: "ws://192.168.178.100:8787" },
    ],
    1_900_000_000,
  );
  if (!built.ok) throw new Error("the maximum invite must build");
  return built.text;
}

const qrVersion = (shape: QrShape): number => (shape.size - 2 * QR_MARGIN - 17) / 4;

/**
 * What a scanner sees when the screen shows the code the way the app does: light modules on a
 * black ground, the modules at `brightness` of full white (the app's default is about 50 %).
 */
// 4 px per module is a typical camera frame; zxing-js is erratic on pixel-perfect synthetic images
// at some other scales (its alignment search), which says nothing about the code.
function renderInverted(
  shape: QrShape,
  brightness: number,
  pixelsPerModule = 4,
): RGBLuminanceSource {
  const side = shape.size * pixelsPerModule;
  const luminance = new Uint8ClampedArray(side * side); // black ground
  const module = Math.round(255 * brightness);
  for (const run of shape.path.matchAll(/M(\d+) (\d+)h(\d+)/g)) {
    const [x, y, width] = [Number(run[1]), Number(run[2]), Number(run[3])];
    for (let py = y * pixelsPerModule; py < (y + 1) * pixelsPerModule; py++) {
      luminance.fill(
        module,
        py * side + x * pixelsPerModule,
        py * side + (x + width) * pixelsPerModule,
      );
    }
  }
  return new RGBLuminanceSource(luminance, side, side);
}

function scan(source: RGBLuminanceSource): string {
  // Phones read light-on-dark codes by also trying the inverted image; so does this decoder.
  const bitmap = new BinaryBitmap(new HybridBinarizer(new InvertedLuminanceSource(source)));
  return new QRCodeReader().decode(bitmap).getText();
}

const invite = maxInvite();

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

describe("the v4 invite code", () => {
  it("stays within 300 bytes and about QR version 13 at level M", () => {
    expect(invite.length).toBeGreaterThan(250);
    expect(invite.length).toBeLessThanOrEqual(300);
    expect(qrVersion(qrShape(invite))).toBeLessThanOrEqual(13);
  });

  it.each([0.5, 1])("decodes when shown inverted at %s of full brightness", (brightness) => {
    const shape = qrShape(invite);
    expect(scan(renderInverted(shape, brightness))).toBe(invite);
  });

  it("decodes a code of exactly 300 bytes (QR version 13)", () => {
    // Labels of at most 50 characters keep the host a valid DNS name at any length.
    const host = (n: number): string =>
      Array.from({ length: n }, (_, i) => ((i + 1) % 51 === 0 ? "." : "a")).join("");
    const make = (n: number): string =>
      JSON.stringify({ v: 4, k: "A".repeat(43), m: [`n:${host(n)}.example`], x: 1_900_000_000 });
    const text = make(300 - make(0).length);
    expect(text.length).toBe(300);
    expect(qrVersion(qrShape(text))).toBe(13);
    expect(scan(renderInverted(qrShape(text), 0.5))).toBe(text);
  });
});
