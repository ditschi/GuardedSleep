import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { chromium } from "@playwright/test";
import { parentDeepLink, startSession } from "../utils/pairing";
import {
  E2E_MIN_LOUD_MAX_RMS,
  waitForConnected,
  waitForParentRemoteAudioAbove,
} from "../utils/probes";
import { hybridSignalWs, launchHybridBaby, setupHybridAndroidApk } from "../utils/hybrid-android";

const hybridEnabled = process.env.GUARDEDSLEEP_HYBRID_E2E === "1";

test.describe("hybrid: Android baby + browser parent", () => {
  test.skip(!hybridEnabled, "Set GUARDEDSLEEP_HYBRID_E2E=1 with a running Android emulator");

  test("noise alert path delivers non-silent audio to browser parent", async () => {
    test.setTimeout(180_000);

    setupHybridAndroidApk();

    const room = `hybrid-${randomUUID().slice(0, 8)}`;
    launchHybridBaby(room);

    const headless = process.env.CI === "true" || process.env.HEADLESS !== "0";
    const browser = await chromium.launch({ headless });
    const parent = await browser.newContext({
      baseURL: "http://127.0.0.1:5173",
      ignoreHTTPSErrors: true,
      permissions: ["microphone"],
    });
    const parentPage = await parent.newPage();
    await parentPage.addInitScript((url: string) => {
      localStorage.setItem("guardedsleep:signaling-url", url);
    }, hybridSignalWs);

    try {
      await parentPage.goto(parentDeepLink(room));
      await startSession(parentPage);
      await waitForConnected(parentPage, 60_000);

      await parentPage.evaluate(() => window.__guardedsleep_test!.waitNextNoiseAlert!(45_000));

      const sample = await waitForParentRemoteAudioAbove(parentPage, E2E_MIN_LOUD_MAX_RMS, 35_000, {
        windowMs: 900,
        intervalMs: 80,
      });
      expect(sample.maxRms).toBeGreaterThanOrEqual(E2E_MIN_LOUD_MAX_RMS);
      expect(sample.samples).toBeGreaterThan(0);
    } finally {
      await parentPage.close().catch(() => {});
      await parent.close().catch(() => {});
      await browser.close().catch(() => {});
    }
  });
});
