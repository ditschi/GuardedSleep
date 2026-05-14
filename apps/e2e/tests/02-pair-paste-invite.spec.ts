import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { startSession } from "./utils/pairing";
import { waitDataChannelOpen, waitForConnected, waitForPairingVerified } from "./utils/probes";

test("pair UX: pasted compact invite completes pair_proof", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto("/");
    await dual.babyPage.getByTestId("role-baby").click();
    await dual.babyPage.waitForURL(/\/pair\?role=baby/);
    await dual.babyPage.getByTestId("pair-start").click();
    await dual.babyPage.waitForURL(/\/baby\?/);

    const invite = await dual.babyPage.evaluate(
      () => window.__guardedsleep_test?.getInviteJson?.() ?? "",
    );
    expect(invite.length).toBeGreaterThan(20);

    await startSession(dual.babyPage);

    await dual.parentPage.goto("/");
    await dual.parentPage.getByTestId("role-parent").click();
    await dual.parentPage.waitForURL(/\/pair\?role=parent/);
    await dual.parentPage.getByTestId("parent-join-field").fill(invite);
    await dual.parentPage.keyboard.press("Enter");
    await dual.parentPage.waitForURL(/\/parent\?/);

    await startSession(dual.parentPage);

    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);
    await waitDataChannelOpen(dual.babyPage);
    await waitDataChannelOpen(dual.parentPage);

    await waitForPairingVerified(dual.babyPage);
    await waitForPairingVerified(dual.parentPage);
  } finally {
    await dual.dispose();
  }
});
