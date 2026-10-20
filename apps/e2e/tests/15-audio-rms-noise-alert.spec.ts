import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import {
  E2E_MIN_LOUD_MAX_RMS,
  parentSendListen,
  readBabyLocalAudioTrackEnabled,
  readInboundAudioStats,
  waitForConnected,
  waitForParentRemoteAudioAbove,
} from "./utils/probes";

test("audio RMS: noise alert enables baby track and parent hears non-silence", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "pulses-30s.wav"),
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

    await dual.babyPage.waitForFunction(
      () =>
        window.__guardedsleep_test?.getSession?.()?.getE2eBabyLocalAudioTrackEnabled?.() === true,
      undefined,
      { timeout: 10_000 },
    );
    expect(await readBabyLocalAudioTrackEnabled(dual.babyPage)).toBe(true);

    await parentSendListen(dual.parentPage, true);

    let packets = 0;
    const rtpDeadline = Date.now() + 20_000;
    while (Date.now() < rtpDeadline) {
      const s = await readInboundAudioStats(dual.parentPage);
      packets = s?.packetsReceived ?? 0;
      if (packets > 8) break;
      await new Promise((r) => setTimeout(r, 250));
    }
    expect(packets).toBeGreaterThan(8);

    await dual.parentPage.evaluate(() => window.__guardedsleep_test?.resetListenAudioForE2e?.());

    const sample = await waitForParentRemoteAudioAbove(
      dual.parentPage,
      E2E_MIN_LOUD_MAX_RMS,
      45_000,
      { windowMs: 1200, intervalMs: 80 },
    );
    expect(sample.maxRms, `parent maxRms=${sample.maxRms}`).toBeGreaterThanOrEqual(
      E2E_MIN_LOUD_MAX_RMS,
    );
    expect(sample.samples).toBeGreaterThan(0);
  } finally {
    await dual.dispose();
  }
});
