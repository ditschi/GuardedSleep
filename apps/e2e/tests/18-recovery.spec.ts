import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { readBar, waitForConnected, waitForLevel } from "./utils/probes";

/**
 * Cuts the parent off the network: the context goes offline (signaling cannot reconnect) and the
 * live peer connection is closed, because Chromium's offline emulation does not touch WebRTC
 * media. The core then has to find out by itself (liveness timeout), escalate, and recover.
 */
async function cutParent(context: BrowserContext, page: Page): Promise<void> {
  await context.setOffline(true);
  await page.evaluate(() => {
    window.__guardedsleep_test?.getSession?.()?.getE2ePeerConnection?.()?.close();
  });
}

async function hidePage(page: Page, hidden: boolean): Promise<void> {
  await page.evaluate((h) => {
    Object.defineProperty(document, "visibilityState", {
      value: h ? "hidden" : "visible",
      configurable: true,
    });
    Object.defineProperty(document, "hidden", { value: h, configurable: true });
    document.dispatchEvent(new Event("visibilitychange"));
  }, hidden);
}

for (const hidden of [false, true]) {
  const suffix = hidden ? " (parent page hidden)" : "";

  test(`recovery: parent offline for 20 s never reaches level 2 and reconnects${suffix}`, async () => {
    test.setTimeout(150_000);
    const dual = await launchDualContexts({
      babyFakeWav: join(fixturesDir, "silence-30s.wav"),
      parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    });
    try {
      await pairDual(dual);
      await waitForConnected(dual.babyPage);
      await waitForConnected(dual.parentPage);
      let maxLevel = 0;
      await cutParent(dual.parent, dual.parentPage);
      if (hidden) await hidePage(dual.parentPage, true);
      const until = Date.now() + 20_000;
      while (Date.now() < until) {
        maxLevel = Math.max(maxLevel, (await readBar(dual.parentPage)).level ?? 0);
        await new Promise((r) => setTimeout(r, 500));
      }
      await dual.parent.setOffline(false);
      if (hidden) await hidePage(dual.parentPage, false);
      await waitForConnected(dual.parentPage, 60_000);
      await waitForConnected(dual.babyPage, 60_000);
      maxLevel = Math.max(maxLevel, 0);
      expect(maxLevel).toBeLessThan(2);
      if (hidden) {
        await expect(dual.parentPage.getByTestId("parent_background_warning")).toBeVisible();
      }
    } finally {
      await dual.dispose();
    }
  });

  test(`recovery: parent offline for 90 s shows level 2, then is restored${suffix}`, async () => {
    test.setTimeout(240_000);
    const dual = await launchDualContexts({
      babyFakeWav: join(fixturesDir, "silence-30s.wav"),
      parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    });
    try {
      await pairDual(dual);
      await waitForConnected(dual.babyPage);
      await waitForConnected(dual.parentPage);
      await cutParent(dual.parent, dual.parentPage);
      if (hidden) await hidePage(dual.parentPage, true);

      await waitForLevel(dual.parentPage, 2, 100_000);
      await expect(dual.parentPage.getByTestId("link_alarm")).toBeVisible();
      await new Promise((r) => setTimeout(r, Math.max(0, 90_000 - 60_000)));

      const toast = dual.parentPage
        .getByTestId("parent_restored_toast")
        .waitFor({ state: "visible", timeout: 90_000 });
      await dual.parent.setOffline(false);
      if (hidden) await hidePage(dual.parentPage, false);
      await waitForConnected(dual.parentPage, 60_000);
      await waitForConnected(dual.babyPage, 60_000);
      await toast;
    } finally {
      await dual.dispose();
    }
  });
}
