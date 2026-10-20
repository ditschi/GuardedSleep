import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { pairDual } from "./utils/pairing";
import { waitDataChannelOpen, waitForConnected } from "./utils/probes";

test("pairing code: baby and parent reach connected with open DataChannel", async () => {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await pairDual(dual);
    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);
    await waitDataChannelOpen(dual.babyPage);
    await waitDataChannelOpen(dual.parentPage);
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      "connected",
    );
    await expect(dual.parentPage.getByTestId("parent_ambient_bar")).toHaveAttribute(
      "data-state",
      "connected",
    );
  } finally {
    await dual.dispose();
  }
});
