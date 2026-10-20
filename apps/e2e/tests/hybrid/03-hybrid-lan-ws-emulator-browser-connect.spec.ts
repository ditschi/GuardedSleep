import { test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { hybridLanUrls, hybridUseLan } from "../utils/hybrid-android";
import { runHybridLanBrowserParentConnect } from "../utils/hybrid-lan-session";
import { probeHttpReachable, probeWsReachable } from "../utils/hybrid-lan-probe";
import { resolveHybridHostIp } from "../utils/hybrid-host";

const hybridEnabled = process.env.GUARDEDSLEEP_HYBRID_E2E === "1";

test.describe("hybrid: emulator/phone baby + browser parent (LAN ws)", () => {
  test.skip(!hybridEnabled, "Set GUARDEDSLEEP_HYBRID_E2E=1 with adb device attached");
  test.skip(!hybridUseLan, "Set GUARDEDSLEEP_HYBRID_LAN=1 (no adb reverse — real host IP)");

  test("plain WebSocket on host IP reaches connected + open DataChannel", async () => {
    test.setTimeout(300_000);

    const host = resolveHybridHostIp();
    const { devServer, signalUrl } = hybridLanUrls("ws");

    const httpOk = await probeHttpReachable(`${devServer}/`);
    test.skip(
      !httpOk,
      `Dev stack not reachable at ${devServer} — run pnpm run e2e or pnpm run dev:local`,
    );

    const wsOk = await probeWsReachable(signalUrl);
    test.skip(!wsOk, `Signaling not reachable at ${signalUrl}`);

    const room = `hybrid-ws-${randomUUID().slice(0, 8)}`;
    await runHybridLanBrowserParentConnect({
      room,
      devServer,
      signalUrl,
      scheme: "ws",
      label: `hybrid ws ${host}`,
    });
  });
});
