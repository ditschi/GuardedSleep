import { describe, expect, it } from "vitest";
import {
  decodeBase64Url,
  generatePairingCode,
  PAIRING_CODE_TTL_SECONDS,
  parsePairingCode,
} from "./pairing-v3.js";

const NOW = 1_800_000_000_000;
const URL_OK = "wss://signal.example.org/signaling";

function code(over: Record<string, unknown> = {}): string {
  const fresh = generatePairingCode([URL_OK], NOW);
  return JSON.stringify({ ...JSON.parse(fresh.payload), ...over });
}

describe("pairing code v3", () => {
  it("generates a fresh 32-byte PSK per code with a 5-minute expiry", () => {
    const a = generatePairingCode([URL_OK], NOW);
    const b = generatePairingCode([URL_OK], NOW);
    expect(a.material.pskB64Url).not.toBe(b.material.pskB64Url);
    expect(decodeBase64Url(a.material.pskB64Url)?.length).toBe(32);
    const parsed = JSON.parse(a.payload);
    expect(parsed).toEqual({
      v: 3,
      k: a.material.pskB64Url,
      s: [URL_OK],
      x: Math.floor(NOW / 1000) + PAIRING_CODE_TTL_SECONDS,
    });
    expect(a.expiresAtWallMs).toBe(parsed.x * 1000);
  });

  it("round-trips through the parent parser", () => {
    const gen = generatePairingCode([URL_OK, "ws://192.168.1.5:8787"], NOW);
    const r = parsePairingCode(gen.payload, NOW + 1000);
    expect(r).toEqual({
      ok: true,
      material: gen.material,
      expiresAtSeconds: gen.expiresAtWallMs / 1000,
    });
  });

  it("reports an expired code", () => {
    const r = parsePairingCode(code(), NOW + (PAIRING_CODE_TTL_SECONDS + 1) * 1000);
    expect(r).toEqual({ ok: false, error: "expired" });
  });

  it.each([
    ["plain text", "hello", "not_our_code"],
    ["a web link", "https://example.org/pair", "not_our_code"],
    ["another JSON", '{"foo":1}', "not_our_code"],
    ["old v1 invite", '{"v":1,"r":"abc","s":"ws://x","k":"AA","t":1}', "not_our_code"],
    ["json array", "[1,2]", "not_our_code"],
    ["empty", "   ", "incomplete_paste"],
    ["cut-off json", code().slice(0, 40), "incomplete_paste"],
  ])("rejects %s", (_name, text, error) => {
    expect(parsePairingCode(text, NOW + 1000)).toEqual({ ok: false, error });
  });

  it.each([
    ["short key", { k: "AAAA" }],
    ["bad alphabet", { k: "!!!!" }],
    ["missing urls", { s: undefined }],
  ])("treats a v3 code with a damaged %s as incomplete", (_name, over) => {
    const r = parsePairingCode(code(over), NOW + 1000);
    expect(r).toEqual({ ok: false, error: "incomplete_paste" });
  });

  it.each([
    ["no urls", { s: [] }],
    ["http url", { s: ["http://x"] }],
    ["too many urls", { s: Array.from({ length: 5 }, (_, i) => `ws://h${i}:1`) }],
    ["non-string url", { s: [1] }],
  ])("rejects a code with %s", (_name, over) => {
    const r = parsePairingCode(code(over), NOW + 1000);
    expect(r).toEqual({ ok: false, error: "not_our_code" });
  });

  it("drops invalid signaling URLs when generating", () => {
    const gen = generatePairingCode(["nonsense", URL_OK], NOW);
    expect(gen.material.signalingUrls).toEqual([URL_OK]);
  });
});
