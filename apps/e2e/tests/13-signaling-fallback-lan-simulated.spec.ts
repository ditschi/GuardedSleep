import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { createServer, type Server as HttpServer } from "node:http";
import { AddressInfo } from "node:net";
import { join } from "node:path";
import { WebSocketServer } from "ws";
import { attachSignalingServer } from "../../signaling-server/dist/server.js";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { babyDeepLink, parentDeepLink, startSession } from "./utils/pairing";
import { waitForConnected } from "./utils/probes";

/**
 * ADR-012 acceptance test: the primary signaling URL is unreachable (simulates
 * "no internet"), but a second signaling endpoint reachable on the same machine
 * substitutes for the "embedded WS listener on the LAN" -- the path-selector
 * must pick it via the saved-connection `fallbackSignalingUrls` and the session
 * must come up regardless of the primary being down.
 *
 * Why this test exists vs. real NSD/Android: see ADR-012 §Consequences/Risks.
 * Playwright cannot exercise Android NSD; this test pins the orchestration
 * layer (path-selection + saved-connection fallbacks) that the Android
 * embedded-server work in Phase 2 will plug into.
 */
async function startFallbackSignalingServer(): Promise<{
  url: string;
  close: () => Promise<void>;
}> {
  const httpServer: HttpServer = createServer((_req, res) => {
    res.writeHead(200, { "content-type": "text/plain" });
    res.end("e2e fallback signaling\n");
  });
  const wss = new WebSocketServer({ server: httpServer });
  attachSignalingServer(wss);
  await new Promise<void>((resolve) => httpServer.listen(0, "127.0.0.1", resolve));
  const addr = httpServer.address() as AddressInfo;
  const url = `ws://127.0.0.1:${addr.port}`;
  return {
    url,
    close: () =>
      new Promise<void>((resolve) => {
        wss.close(() => httpServer.close(() => resolve()));
      }),
  };
}

test("offline-LAN simulation: primary signaling unreachable, fallback wins", async () => {
  const room = `e2e-${randomUUID().slice(0, 8)}`;
  // Use an unreachable port for the "internet" signaling -- guaranteed to time out / refuse.
  const unreachablePrimary = "ws://127.0.0.1:19999";
  const fallback = await startFallbackSignalingServer();

  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    // Seed a saved connection on BOTH contexts with `fallbackSignalingUrls`
    // pointing at the local substitute. This mirrors the production flow where
    // saved connections carry the LAN-host fallbacks.
    const seedPayload = {
      primary: unreachablePrimary,
      fb: fallback.url,
      roomId: room,
    };
    const seedSaved = (payload: { primary: string; fb: string; roomId: string }) => {
      const row = {
        id: "e2e-seed",
        roomId: payload.roomId,
        signalingUrl: payload.primary,
        fallbackSignalingUrls: [payload.fb],
        lastUsedAt: Date.now(),
        label: "e2e",
      };
      localStorage.setItem("guardedsleep:saved-connections", JSON.stringify([row]));
    };
    await dual.babyPage.addInitScript(seedSaved, seedPayload);
    await dual.parentPage.addInitScript(seedSaved, seedPayload);

    await dual.babyPage.goto(babyDeepLink(room, unreachablePrimary));
    await dual.parentPage.goto(parentDeepLink(room, unreachablePrimary));

    await startSession(dual.babyPage);
    await startSession(dual.parentPage);

    await waitForConnected(dual.babyPage, 30_000);
    await waitForConnected(dual.parentPage, 30_000);

    // Ambient bar must show the green "Connected" state on both ends.
    const babyBar = dual.babyPage.getByTestId("connection-status-bar");
    const parentBar = dual.parentPage.getByTestId("connection-status-bar");
    await expect(babyBar).toHaveAttribute("data-status", "connected");
    await expect(parentBar).toHaveAttribute("data-status", "connected");
  } finally {
    await dual.dispose();
    await fallback.close();
  }
});
