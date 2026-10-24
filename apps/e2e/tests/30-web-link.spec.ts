import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import { join } from "node:path";
import { pageGuardStub, setHidden, wakeLockStub } from "./utils/browser-stubs";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import {
  closePeerConnections,
  dropDataChannels,
  peerConnectionCount,
  rtcTrackerStub,
} from "./utils/rtc";
import { CLIPBOARD_PERMISSIONS, pairWeb, showCode, waitForBarKind } from "./utils/web";

/**
 * Link behaviour from the user's side: a wrong key and an unreachable server while pairing, a
 * dropped DataChannel, and outages of the parent device. The core decides when to escalate; these
 * tests only look at what the screens show. The parent's address list comes from the code, so
 * the pairing cases edit the code the way a damaged or foreign one would look.
 */

const silence = join(fixturesDir, "silence-30s.wav");
// A mailbox entry; the dev build of the page accepts loopback, a release build would not.
const DEAD_MAILBOX = "l:127.0.0.1:19999";

type Invite = { v: number; k: string; m: string[]; x: number };

async function pasteInto(page: Page, code: string): Promise<void> {
  await page.goto("/");
  await page.getByTestId("role_parent").click();
  await page.getByTestId("parent_pair_paste_field").fill(code);
  await page.getByTestId("parent_pair_paste_submit").click();
}

test("a code with a wrong key never pairs: the parent says the baby device cannot be reached", async () => {
  test.setTimeout(120_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    const invite = JSON.parse(await showCode(dual.babyPage)) as Invite;
    // A different key is a different room: the two devices never meet.
    const index = 10;
    const flipped = invite.k[index] === "A" ? "B" : "A";
    invite.k = `${invite.k.slice(0, index)}${flipped}${invite.k.slice(index + 1)}`;
    await pasteInto(dual.parentPage, JSON.stringify(invite));

    await expect(dual.parentPage.getByTestId("parent_pair_connecting")).toBeVisible();
    await expect(dual.parentPage.getByTestId("parent_pair_error")).toBeVisible({ timeout: 60_000 });
    await expect(dual.parentPage).toHaveURL(/\/parent\/pair$/);
    await expect(dual.babyPage.getByTestId("baby_pair_qr")).toBeVisible();
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveCount(0);
  } finally {
    await dual.dispose();
  }
});

test("a code whose only server is unreachable says so, and a dead first address is skipped", async () => {
  test.setTimeout(150_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    const invite = JSON.parse(await showCode(dual.babyPage)) as Invite;

    // Only a dead address: the parent keeps trying and names the server after 20 s.
    await pasteInto(dual.parentPage, JSON.stringify({ ...invite, m: [DEAD_MAILBOX] }));
    await expect(dual.parentPage.getByTestId("parent_pair_error")).toBeVisible({ timeout: 60_000 });
    await dual.parentPage.getByTestId("parent_pair_back").click();

    // The same code with the dead address in front: the driver moves on to the real one.
    await pasteInto(dual.parentPage, JSON.stringify({ ...invite, m: [DEAD_MAILBOX, ...invite.m] }));
    await waitForBarKind(dual.parentPage, "connected", 90_000);
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 60_000 },
    );
  } finally {
    await dual.dispose();
  }
});

test("a dropped DataChannel is recovered by the core without user action", async () => {
  test.setTimeout(150_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await dual.baby.addInitScript(rtcTrackerStub);
    await dual.parent.addInitScript(rtcTrackerStub);
    await pairWeb(dual);
    const before = await peerConnectionCount(dual.babyPage);

    await dropDataChannels(dual.babyPage);

    // A new connection replaces the broken one and both devices show the link again.
    await expect
      .poll(() => peerConnectionCount(dual.babyPage), { timeout: 60_000 })
      .toBeGreaterThan(before);
    await waitForBarKind(dual.parentPage, "connected", 90_000);
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 60_000 },
    );
  } finally {
    await dual.dispose();
  }
});

/**
 * Cuts the parent off the network: the context goes offline (signaling cannot reconnect) and the
 * live peer connection is closed, because Chromium's offline emulation does not touch WebRTC
 * media. The core then has to find out by itself (liveness timeout), escalate, and recover.
 */
async function cutParent(context: BrowserContext, page: Page): Promise<void> {
  await context.setOffline(true);
  await closePeerConnections(page);
}

for (const hidden of [false, true]) {
  const suffix = hidden ? " (parent page hidden)" : "";

  test(`recovery: parent offline for 20 s never raises the link alarm and reconnects${suffix}`, async () => {
    test.setTimeout(150_000);
    const dual = await launchDualContexts({
      babyFakeWav: silence,
      parentFakeWav: silence,
      permissions: CLIPBOARD_PERMISSIONS,
    });
    const parent = dual.parentPage;
    try {
      for (const ctx of [dual.baby, dual.parent]) {
        await ctx.addInitScript(rtcTrackerStub);
        // A held wake lock keeps its own warnings out of this test.
        await ctx.addInitScript(wakeLockStub);
        await ctx.addInitScript(pageGuardStub);
      }
      await pairWeb(dual);

      await cutParent(dual.parent, parent);
      if (hidden) await setHidden(parent, true);
      let alarmSeen = false;
      const until = Date.now() + 20_000;
      while (Date.now() < until) {
        alarmSeen ||= (await parent.getByTestId("parent_link_alarm").count()) > 0;
        await parent.waitForTimeout(500);
      }
      await dual.parent.setOffline(false);
      if (hidden) await setHidden(parent, false);

      await waitForBarKind(parent, "connected", 90_000);
      await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
        "data-state",
        "connected",
        { timeout: 60_000 },
      );
      expect(alarmSeen, "a short outage stays below the alarm").toBe(false);
      if (hidden) await expect(parent.getByTestId("away_notice")).toBeVisible();
    } finally {
      await dual.dispose();
    }
  });
}

test("recovery: parent offline for 90 s raises the link alarm naming the server, then is restored", async () => {
  test.setTimeout(240_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const parent = dual.parentPage;
  try {
    await dual.baby.addInitScript(rtcTrackerStub);
    await dual.parent.addInitScript(rtcTrackerStub);
    await pairWeb(dual);

    await cutParent(dual.parent, parent);
    await expect(parent.getByTestId("parent_link_alarm")).toBeVisible({ timeout: 120_000 });
    await expect(parent.getByTestId("link_alarm_cause")).toHaveAttribute(
      "data-cause",
      "server_unreachable",
      { timeout: 60_000 },
    );
    await expect(parent.getByTestId("link_alarm_ack")).toBeVisible();
    await parent.waitForTimeout(25_000);

    await dual.parent.setOffline(false);
    await expect(parent.getByTestId("parent_restored_toast")).toBeVisible({ timeout: 90_000 });
    await waitForBarKind(parent, "connected", 60_000);
    await expect(parent.getByTestId("parent_link_alarm")).toHaveCount(0);
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 60_000 },
    );
  } finally {
    await dual.dispose();
  }
});
