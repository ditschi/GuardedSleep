import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pasteInvite, readInvite, startSession } from "./utils/pairing";
import { waitDataChannelOpen, waitForConnected, waitForPairingVerified } from "./utils/probes";

/**
 * Standard v3 pairing UX: the baby unit shows a code (QR, countdown, copyable text) while it
 * waits; the parent pastes it; the baby hides the code once the parent is verified.
 */
test("standard flow: baby shows code while waiting, parent joins, baby leaves the code view", async () => {
  test.setTimeout(120_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto("/baby");
    await expect(dual.babyPage.getByTestId("setup_title")).toBeVisible();
    await startSession(dual.babyPage);

    await expect(dual.babyPage.getByTestId("baby_pair_qr")).toBeVisible({ timeout: 15_000 });
    await expect(dual.babyPage.getByTestId("baby_pair_countdown")).toBeVisible();
    await expect(dual.babyPage.getByTestId("baby_pair_waiting")).toBeVisible();
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      /^(connecting|connected)$/,
    );

    const invite = await readInvite(dual.babyPage);
    const code = JSON.parse(invite) as { v: number; k: string; s: string[]; x: number };
    expect(code.v).toBe(3);
    expect(Buffer.from(code.k, "base64url")).toHaveLength(32);
    expect(code.x * 1000).toBeGreaterThan(Date.now());
    expect(code.x * 1000).toBeLessThanOrEqual(Date.now() + 300_000);

    await pasteInvite(dual.parentPage, invite);

    await waitForConnected(dual.babyPage, 60_000);
    await waitForConnected(dual.parentPage, 60_000);
    await waitDataChannelOpen(dual.babyPage);
    await waitDataChannelOpen(dual.parentPage);
    await Promise.all([
      waitForPairingVerified(dual.babyPage),
      waitForPairingVerified(dual.parentPage),
    ]);

    await expect(dual.babyPage.getByTestId("baby_pair_qr")).toBeHidden();
    await expect(dual.babyPage.getByTestId("baby_monitoring_line")).toBeVisible();
    await expect(dual.babyPage.getByTestId("baby_silence")).toBeVisible();
    await expect(dual.parentPage.getByTestId("parent_silence_chip")).toHaveAttribute(
      "data-silence",
      "unknown",
    );
    await expect(dual.parentPage.getByTestId("parent_keep_open_banner")).toBeVisible();
    // PSK in memory only: nothing in web storage.
    for (const page of [dual.babyPage, dual.parentPage]) {
      const leaked = await page.evaluate((k) => {
        const all = [localStorage, sessionStorage].flatMap((st) =>
          Object.keys(st).map((key) => `${key}=${st.getItem(key)}`),
        );
        return all.some((row) => row.includes(k));
      }, code.k);
      expect(leaked).toBe(false);
    }
  } finally {
    await dual.dispose();
  }
});
