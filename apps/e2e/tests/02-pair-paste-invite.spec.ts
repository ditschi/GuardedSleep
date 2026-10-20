import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { waitDataChannelOpen, waitForConnected, waitForPairingVerified } from "./utils/probes";

test("pair UX: pasted compact invite completes pair_proof", async () => {
  test.setTimeout(120_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    const invite = await pairDual(dual);
    expect(JSON.parse(invite)).toMatchObject({ v: 3 });

    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);
    await waitDataChannelOpen(dual.babyPage);
    await waitDataChannelOpen(dual.parentPage);

    await Promise.all([
      waitForPairingVerified(dual.babyPage),
      waitForPairingVerified(dual.parentPage),
    ]);
  } finally {
    await dual.dispose();
  }
});
