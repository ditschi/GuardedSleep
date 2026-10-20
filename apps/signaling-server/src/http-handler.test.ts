import { createServer } from "node:http";
import type { AddressInfo } from "node:net";
import { once } from "node:events";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { handleHttpRequest } from "./http-handler.js";

describe("handleHttpRequest", () => {
  const server = createServer(handleHttpRequest);
  let base = "";

  beforeAll(async () => {
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  });

  afterAll(async () => {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  });

  it("answers /healthz with ok", async () => {
    const res = await fetch(`${base}/healthz`);
    expect(res.status).toBe(200);
    expect(await res.text()).toBe("ok\n");
    expect(res.headers.get("cache-control")).toBe("no-store");
  });

  it("answers /healthz under a reverse-proxy path prefix", async () => {
    const res = await fetch(`${base}/signaling/healthz?probe=1`);
    expect(await res.text()).toBe("ok\n");
  });

  it("keeps the banner on other paths", async () => {
    const res = await fetch(`${base}/`);
    expect(res.status).toBe(200);
    expect(await res.text()).toBe("GuardedSleep signaling\n");
    expect(res.headers.get("x-frame-options")).toBe("DENY");
  });
});
