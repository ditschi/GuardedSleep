import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { waitForConnected } from "./utils/probes";

test("noise: silence with moderate gate does not alert parent", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual);

    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);

    await dual.parentPage.evaluate(() => {
      window.__guardedsleep_test?.resetNoiseAlertCount?.();
    });

    await new Promise((r) => setTimeout(r, 9000));

    const n = await dual.parentPage.evaluate(
      () => window.__guardedsleep_test?.getNoiseAlertCount?.() ?? -1,
    );
    expect(n).toBe(0);
  } finally {
    await dual.dispose();
  }
});
