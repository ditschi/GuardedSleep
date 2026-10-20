import { test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { hybridLanUrls, hybridUseLan } from "../utils/hybrid-android";
import { runHybridLanBrowserParentConnect } from "../utils/hybrid-lan-session";
import { probeHttpReachable, probeWsReachable } from "../utils/hybrid-lan-probe";
import { resolveHybridHostIp } from "../utils/hybrid-host";

const hybridEnabled = process.env.GUARDEDSLEEP_HYBRID_E2E === "1";

test.describe("hybrid: emulator/phone baby + browser parent (LAN wss)", () => {
  test.skip(!hybridEnabled, "Set GUARDEDSLEEP_HYBRID_E2E=1 with adb device attached");
  test.skip(!hybridUseLan, "Set GUARDEDSLEEP_HYBRID_LAN=1 (no adb reverse — real host IP)");

  test("self-signed WSS on Docker stack reaches connected + open DataChannel", async () => {
    test.setTimeout(360_000);

    const host = resolveHybridHostIp();
    const { devServer, signalUrl } = hybridLanUrls("wss");

    const httpsOk = await probeHttpReachable(`${devServer}/`);
    test.skip(
      !httpsOk,
      `Docker HTTPS stack not reachable at ${devServer} — run pnpm run stack:lan first`,
    );

    const wssOk = await probeWsReachable(signalUrl);
    test.skip(!wssOk, `WSS signaling not reachable at ${signalUrl}`);

    const room = `hybrid-wss-${randomUUID().slice(0, 8)}`;
    await runHybridLanBrowserParentConnect({
      room,
      devServer,
      signalUrl,
      scheme: "wss",
      label: `hybrid wss ${host}`,
    });
  });
});
