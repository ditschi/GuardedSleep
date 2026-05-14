import { describe, expect, it } from "vitest";
import { extractSha256FingerprintFromSdp } from "./dtls-fingerprint.js";

describe("extractSha256FingerprintFromSdp", () => {
  it("normalizes fingerprint", () => {
    const sdp = `v=0\r\na=fingerprint:sha-256 AA:BB:CC:01:02\r\n`;
    expect(extractSha256FingerprintFromSdp(sdp)).toBe("aabbcc0102");
  });
});
