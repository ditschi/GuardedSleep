import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { readBar, waitForCause, waitForLevel } from "./utils/probes";

test("signaling: an unreachable server escalates and names the cause, never gives up", async () => {
  test.setTimeout(150_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    signalUrl: "ws://127.0.0.1:19999",
  });
  try {
    await pairDual(dual);
    await waitForLevel(dual.parentPage, 1, 40_000);
    await waitForCause(dual.parentPage, "server_unreachable", 100_000);
    expect((await readBar(dual.parentPage)).level).toBeGreaterThanOrEqual(2);
    await expect(dual.parentPage.getByTestId("link_try_now")).toBeVisible();
  } finally {
    await dual.dispose();
  }
});
