import { expect, chromium, type Page } from "@playwright/test";
import { parentDeepLink, startSession } from "./pairing";
import { logConnectionDiagnostics } from "./connection-diagnostics";
import { waitDataChannelOpen, waitForConnected } from "./probes";
import { launchHybridBaby, setupHybridAndroidApk, type HybridLanScheme } from "./hybrid-android.js";

export type HybridLanConnectOpts = {
  room: string;
  devServer: string;
  signalUrl: string;
  scheme: HybridLanScheme;
  label: string;
};

export async function runHybridLanBrowserParentConnect(opts: HybridLanConnectOpts): Promise<void> {
  setupHybridAndroidApk({ scheme: opts.scheme });
  launchHybridBaby(opts.room, { devServer: opts.devServer, signalUrl: opts.signalUrl });

  const headless = process.env.CI === "true" || process.env.HEADLESS !== "0";
  const browser = await chromium.launch({ headless });
  const parent = await browser.newContext({
    baseURL: opts.devServer,
    ignoreHTTPSErrors: true,
    permissions: ["microphone"],
  });
  const parentPage = await parent.newPage();
  await parentPage.addInitScript((url: string) => {
    localStorage.setItem("guardedsleep:signaling-url", url);
  }, opts.signalUrl);

  try {
    await gotoParent(parentPage, opts.room, opts.signalUrl);
    await startSession(parentPage);

    try {
      await waitForConnected(parentPage, 90_000);
      await waitDataChannelOpen(parentPage, 45_000);
    } catch (err) {
      await logConnectionDiagnostics(opts.label, parentPage, parentPage);
      throw err;
    }

    const diag = await parentPage.evaluate(
      () => window.__guardedsleep_test?.getConnectionDiagnostics?.() ?? null,
    );
    expect(diag?.effectiveSignalingUrl).toBe(opts.signalUrl);
    expect(diag?.peerConnection?.connectionState).toBe("connected");
    expect(diag?.dataChannelState).toBe("open");
  } finally {
    await parentPage.close().catch(() => {});
    await parent.close().catch(() => {});
    await browser.close().catch(() => {});
  }
}

async function gotoParent(page: Page, room: string, signalUrl: string): Promise<void> {
  await page.goto(parentDeepLink(room, signalUrl));
}
