import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { babyDeepLink, parentDeepLink, startSession } from "./utils/pairing";
import { waitForEitherFailed } from "./utils/probes";

test("signaling: unreachable WebSocket URL yields failure", async () => {
  const room = `e2e-${randomUUID().slice(0, 8)}`;
  const badSignal = "ws://127.0.0.1:19999";
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto(babyDeepLink(room, badSignal));
    await dual.parentPage.goto(parentDeepLink(room, badSignal));

    await startSession(dual.babyPage);
    await startSession(dual.parentPage);

    await waitForEitherFailed(dual.babyPage, dual.parentPage, 35_000);
  } finally {
    await dual.dispose();
  }
});
