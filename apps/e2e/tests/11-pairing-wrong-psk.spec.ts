import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { startSession } from "./utils/pairing";
import { waitForEitherFailed } from "./utils/probes";

function corruptInvitePsk(inviteJson: string): string {
  const o = JSON.parse(inviteJson) as { k: string };
  const k = o.k;
  const last = k.slice(-1);
  const flip = last === "a" ? "b" : "a";
  o.k = `${k.slice(0, -1)}${flip}`;
  return JSON.stringify(o);
}

test("pairing: wrong PSK in pasted invite leads to failure (pair_proof)", async () => {
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
    const badInvite = corruptInvitePsk(invite);

    await startSession(dual.babyPage);

    await dual.parentPage.goto("/");
    await dual.parentPage.getByTestId("role-parent").click();
    await dual.parentPage.waitForURL(/\/pair\?role=parent/);
    await dual.parentPage.getByTestId("parent-join-field").fill(badInvite);
    await dual.parentPage.keyboard.press("Enter");
    await dual.parentPage.waitForURL(/\/parent\?/);

    await startSession(dual.parentPage);

    await waitForEitherFailed(dual.babyPage, dual.parentPage, 40_000);
  } finally {
    await dual.dispose();
  }
});
