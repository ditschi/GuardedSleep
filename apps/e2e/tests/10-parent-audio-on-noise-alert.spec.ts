import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { parentAudioHasSrcObject, waitForConnected } from "./utils/probes";

test("parent: noise alert wires audio element for playback", async () => {
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

    await noiseWait;

    expect(await parentAudioHasSrcObject(dual.parentPage)).toBe(true);
  } finally {
    await dual.dispose();
  }
});
