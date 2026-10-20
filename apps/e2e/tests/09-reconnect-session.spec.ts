import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { waitDataChannelOpen, waitForConnected } from "./utils/probes";

test("session: a dropped DataChannel is recovered by the core without user action", async () => {
  test.setTimeout(120_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual);
    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);
    await waitDataChannelOpen(dual.babyPage);

    await dual.babyPage.evaluate(() => {
      const w = window as unknown as { __oldPc?: RTCPeerConnection | null };
      const s = window.__guardedsleep_test!.getSession!()!;
      w.__oldPc = s.getE2ePeerConnection();
      s.getE2eDataChannel()?.close();
    });

    for (const page of [dual.babyPage, dual.parentPage]) {
      await page
        .waitForFunction(
          () => {
            const w = window as unknown as { __oldPc?: RTCPeerConnection | null };
            const pc = window.__guardedsleep_test?.getSession?.()?.getE2ePeerConnection?.() ?? null;
            return pc !== null && pc.connectionState === "connected" && pc !== (w.__oldPc ?? null);
          },
          undefined,
          { timeout: 60_000 },
        )
        .catch(() => {});
    }
    await waitForConnected(dual.babyPage, 50_000);
    await waitForConnected(dual.parentPage, 50_000);
    await expect(dual.parentPage.getByTestId("parent_ambient_bar")).toHaveAttribute(
      "data-state",
      "connected",
    );
  } finally {
    await dual.dispose();
  }
});
