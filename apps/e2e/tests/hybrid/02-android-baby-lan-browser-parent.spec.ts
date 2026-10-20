import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { chromium } from "@playwright/test";
import { parentDeepLink, startSession } from "../utils/pairing";
import {
  E2E_MIN_LOUD_MAX_RMS,
  waitForConnected,
  waitForParentRemoteAudioAbove,
} from "../utils/probes";
import {
  hybridLanUrls,
  hybridUseLan,
  launchHybridBaby,
  setupHybridAndroidApk,
} from "../utils/hybrid-android";

const hybridEnabled = process.env.GUARDEDSLEEP_HYBRID_E2E === "1";

test.describe("hybrid: Android baby + browser parent (LAN IP)", () => {
  test.skip(!hybridEnabled, "Set GUARDEDSLEEP_HYBRID_E2E=1 with adb device attached");
  test.skip(
    !hybridUseLan,
    "Set GUARDEDSLEEP_HYBRID_LAN=1 to exercise real Wi-Fi URLs (no adb reverse)",
  );

  test("physical phone path: LAN dev server + signaling reach browser parent", async () => {
    test.setTimeout(240_000);

    const { devServer, signalUrl } = hybridLanUrls("ws");
    setupHybridAndroidApk({ scheme: "ws" });
    const room = `hybrid-lan-${randomUUID().slice(0, 8)}`;
    launchHybridBaby(room, { devServer, signalUrl, scheme: "ws" });

    const headless = process.env.CI === "true" || process.env.HEADLESS !== "0";
    const browser = await chromium.launch({ headless });
    const parent = await browser.newContext({
      baseURL: devServer,
      ignoreHTTPSErrors: true,
      permissions: ["microphone"],
    });
    const parentPage = await parent.newPage();
    await parentPage.addInitScript((url: string) => {
      localStorage.setItem("guardedsleep:signaling-url", url);
    }, signalUrl);

    try {
      await parentPage.goto(parentDeepLink(room, signalUrl));
      await startSession(parentPage);
      await waitForConnected(parentPage, 90_000);

      await parentPage.evaluate(() => window.__guardedsleep_test!.waitNextNoiseAlert!(60_000));

      const sample = await waitForParentRemoteAudioAbove(parentPage, E2E_MIN_LOUD_MAX_RMS, 45_000, {
        windowMs: 900,
        intervalMs: 80,
      });
      expect(sample.maxRms).toBeGreaterThanOrEqual(E2E_MIN_LOUD_MAX_RMS);
      expect(sample.samples).toBeGreaterThan(0);

      const diag = await parentPage.evaluate(
        () => window.__guardedsleep_test?.getConnectionDiagnostics?.() ?? null,
      );
      expect(diag?.effectiveSignalingUrl).toBe(signalUrl);
    } finally {
      await parentPage.close().catch(() => {});
      await parent.close().catch(() => {});
      await browser.close().catch(() => {});
    }
  });
});
