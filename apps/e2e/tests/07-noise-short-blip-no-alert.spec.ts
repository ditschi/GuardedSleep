import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { babyDeepLink, parentDeepLink, startSession } from "./utils/pairing";
import { waitForConnected } from "./utils/probes";

test("noise: 50 ms blip does not satisfy hold duration — no alert", async () => {
  const room = `e2e-${randomUUID().slice(0, 8)}`;
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "blip-50ms.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto(babyDeepLink(room));
    await dual.babyPage.evaluate(() => {
      window.__guardedsleep_test?.setNoiseGate?.(0.01, 500);
    });
    await dual.parentPage.goto(parentDeepLink(room));

    await startSession(dual.babyPage);
    await startSession(dual.parentPage);

    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);

    await dual.parentPage.evaluate(() => {
      window.__guardedsleep_test?.resetNoiseAlertCount?.();
    });

    await new Promise((r) => setTimeout(r, 3500));

    const n = await dual.parentPage.evaluate(
      () => window.__guardedsleep_test?.getNoiseAlertCount?.() ?? -1,
    );
    expect(n).toBe(0);
  } finally {
    await dual.dispose();
  }
});
