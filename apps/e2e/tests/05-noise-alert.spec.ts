import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { babyDeepLink, parentDeepLink, startSession } from "./utils/pairing";
import { waitForConnected } from "./utils/probes";

test("noise alert: baby loud audio notifies parent over DataChannel", async () => {
  const room = `e2e-${randomUUID().slice(0, 8)}`;
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "loud-noise.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto(babyDeepLink(room));
    await dual.babyPage.evaluate(() => {
      window.__guardedsleep_test?.setNoiseGate?.(0.15, 30_000);
    });
    await dual.parentPage.goto(parentDeepLink(room));

    await startSession(dual.babyPage);
    await startSession(dual.parentPage);

    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);

    const noiseWait = dual.parentPage.evaluate(() =>
      window.__guardedsleep_test!.waitNextNoiseAlert!(20_000),
    );

    await dual.babyPage.evaluate(() => {
      window.__guardedsleep_test?.setNoiseGate?.(0, 150);
    });

    const rms = await noiseWait;
    expect(Number.isFinite(rms)).toBe(true);
  } finally {
    await dual.dispose();
  }
});
