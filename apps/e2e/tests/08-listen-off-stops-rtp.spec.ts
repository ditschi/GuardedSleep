import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { parentSendListen, readInboundAudioStats, waitForConnected } from "./utils/probes";

test("audio: Listen off disables baby outgoing track (gate)", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "pulses-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual);
    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);

    await parentSendListen(dual.parentPage, true);

    let baseline = 0;
    const t0 = Date.now() + 20_000;
    while (Date.now() < t0) {
      const s = await readInboundAudioStats(dual.parentPage);
      baseline = s?.packetsReceived ?? 0;
      if (baseline > 40) break;
      await new Promise((r) => setTimeout(r, 200));
    }
    expect(baseline).toBeGreaterThan(20);

    await parentSendListen(dual.parentPage, false);

    await dual.babyPage.waitForFunction(
      () =>
        window.__guardedsleep_test?.getSession?.()?.getE2eBabyLocalAudioTrackEnabled?.() === false,
      undefined,
      { timeout: 10_000 },
    );

    const enabled = await dual.babyPage.evaluate(
      () =>
        window.__guardedsleep_test?.getSession?.()?.getE2eBabyLocalAudioTrackEnabled?.() ?? null,
    );
    expect(enabled).toBe(false);
  } finally {
    await dual.dispose();
  }
});
