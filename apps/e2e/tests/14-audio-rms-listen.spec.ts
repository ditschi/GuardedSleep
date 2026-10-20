import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import {
  E2E_MIN_LOUD_MAX_RMS,
  parentSendListen,
  waitForConnected,
  waitForParentRemoteAudioAbove,
} from "./utils/probes";

test("audio RMS: parent receives non-silent stream when Listen is on", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "pulses-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual);
    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);

    await parentSendListen(dual.parentPage, true);

    const sample = await waitForParentRemoteAudioAbove(
      dual.parentPage,
      E2E_MIN_LOUD_MAX_RMS,
      25_000,
      { windowMs: 900, intervalMs: 80 },
    );
    expect(sample.maxRms).toBeGreaterThanOrEqual(E2E_MIN_LOUD_MAX_RMS);
    expect(sample.samples).toBeGreaterThan(0);
  } finally {
    await dual.dispose();
  }
});
