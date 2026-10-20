import { test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { waitForConnected } from "./utils/probes";

/**
 * The pairing code carries an ordered list of signaling URLs. The parent's first URL is dead
 * (simulating "no internet"); the link driver must move on to the next one by itself.
 */
test("signaling URL list: parent skips an unreachable first URL", async () => {
  test.setTimeout(120_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto("/baby");
    await dual.babyPage.getByTestId("session-start").click();
    await dual.babyPage.waitForFunction(
      () => (window.__guardedsleep_test?.getInviteJson?.() ?? "").length > 20,
    );
    const invite = await dual.babyPage.evaluate(() => window.__guardedsleep_test!.getInviteJson!());
    const code = JSON.parse(invite) as { s: string[] };
    code.s = ["ws://127.0.0.1:19999", ...code.s];
    await dual.parentPage.goto("/pair?role=parent");
    await dual.parentPage.getByTestId("parent_pair_paste_field").fill(JSON.stringify(code));
    await dual.parentPage.getByTestId("parent_pair_paste_submit").click();
    await dual.parentPage.waitForURL(/\/parent$/);
    await dual.parentPage
      .getByTestId("parent_sound_gate")
      .click({ timeout: 5000 })
      .catch(() => {});

    await waitForConnected(dual.babyPage, 60_000);
    await waitForConnected(dual.parentPage, 60_000);
  } finally {
    await dual.dispose();
  }
});
