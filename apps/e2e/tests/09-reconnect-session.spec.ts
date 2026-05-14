import { expect, test } from "@playwright/test";
import { randomUUID } from "node:crypto";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { babyDeepLink, parentDeepLink, startSession } from "./utils/pairing";
import { waitDataChannelOpen, waitForConnected } from "./utils/probes";

test("session: (re)start on both sides reaches connected again", async () => {
  const room = `e2e-${randomUUID().slice(0, 8)}`;
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
  });
  try {
    await dual.babyPage.goto(babyDeepLink(room));
    await dual.parentPage.goto(parentDeepLink(room));
    await startSession(dual.babyPage);
    await startSession(dual.parentPage);
    await waitForConnected(dual.babyPage);
    await waitForConnected(dual.parentPage);
    await waitDataChannelOpen(dual.babyPage);
    await waitDataChannelOpen(dual.parentPage);

    await startSession(dual.babyPage);
    await startSession(dual.parentPage);

    await waitForConnected(dual.babyPage, 50_000);
    await waitForConnected(dual.parentPage, 50_000);
    await waitDataChannelOpen(dual.babyPage, 50_000);
    await waitDataChannelOpen(dual.parentPage, 50_000);

    await expect(dual.babyPage.getByTestId("connection-status")).toHaveAttribute(
      "data-session-state",
      "connected",
    );
    await expect(dual.parentPage.getByTestId("connection-status")).toHaveAttribute(
      "data-session-state",
      "connected",
    );
  } finally {
    await dual.dispose();
  }
});
