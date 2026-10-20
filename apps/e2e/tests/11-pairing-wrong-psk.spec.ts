import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { readInvite, pasteInvite, startSession } from "./utils/pairing";
import { readBar, waitForCause } from "./utils/probes";

/** Changes a character in the middle of the PSK so the decoded key really differs. */
function corruptInvitePsk(inviteJson: string): string {
  const o = JSON.parse(inviteJson) as { k: string };
  const i = 10;
  o.k = `${o.k.slice(0, i)}${o.k[i] === "A" ? "B" : "A"}${o.k.slice(i + 1)}`;
  return JSON.stringify(o);
}

test("pairing: a wrong PSK is a different room, so the phones never pair and the parent shows why", async () => {
  test.setTimeout(150_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto("/baby");
    await startSession(dual.babyPage);
    const invite = await readInvite(dual.babyPage);
    await pasteInvite(dual.parentPage, corruptInvitePsk(invite));

    // Level 2 (60 s without contact) brings the troubleshooter with the cause hint.
    await waitForCause(dual.parentPage, "peer_absent", 100_000);
    expect((await readBar(dual.parentPage)).state).not.toBe("connected");
    expect((await readBar(dual.babyPage)).state).not.toBe("connected");
    await expect(dual.parentPage.getByTestId("link_help")).toBeVisible();
  } finally {
    await dual.dispose();
  }
});
