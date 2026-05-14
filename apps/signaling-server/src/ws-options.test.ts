import { describe, expect, it, afterEach } from "vitest";
import { verifyWebSocketClient } from "./ws-options.js";

describe("verifyWebSocketClient", () => {
  const origOrigins = process.env.SIGNALING_ALLOWED_ORIGINS;

  afterEach(() => {
    if (origOrigins === undefined) delete process.env.SIGNALING_ALLOWED_ORIGINS;
    else process.env.SIGNALING_ALLOWED_ORIGINS = origOrigins;
  });

  it("allows all when SIGNALING_ALLOWED_ORIGINS unset", () => {
    delete process.env.SIGNALING_ALLOWED_ORIGINS;
    verifyWebSocketClient(
      { origin: "https://evil.example", secure: true, req: {} as never },
      (ok) => {
        expect(ok).toBe(true);
      },
    );
  });

  it("allows empty Origin when allowlist is set (native / tooling clients)", () => {
    process.env.SIGNALING_ALLOWED_ORIGINS = "https://app.example";
    verifyWebSocketClient({ origin: "", secure: false, req: {} as never }, (ok) => {
      expect(ok).toBe(true);
    });
  });

  it("allows browser Origin when in allowlist", () => {
    process.env.SIGNALING_ALLOWED_ORIGINS = "https://good.example,https://other.example";
    verifyWebSocketClient(
      { origin: "https://good.example", secure: true, req: {} as never },
      (ok) => {
        expect(ok).toBe(true);
      },
    );
  });

  it("rejects browser Origin when not in allowlist", () => {
    process.env.SIGNALING_ALLOWED_ORIGINS = "https://good.example";
    verifyWebSocketClient(
      { origin: "https://evil.example", secure: true, req: {} as never },
      (ok) => {
        expect(ok).toBe(false);
      },
    );
  });
});
