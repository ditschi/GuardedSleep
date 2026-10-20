import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { waitForConnected } from "./utils/probes";

test("noise alert: baby loud audio notifies parent over DataChannel", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "loud-noise.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual, { syntheticLevels: true });

    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);

    const noiseWait = dual.parentPage.evaluate(() =>
      window.__guardedsleep_test!.waitNextNoiseAlert!(20_000),
    );

    await dual.babyPage.evaluate(() => {
      window.__guardedsleep_test?.injectAudioLevel?.(0.5, 7000);
    });

    const rms = await noiseWait;
    expect(Number.isFinite(rms)).toBe(true);
  } finally {
    await dual.dispose();
  }
});
