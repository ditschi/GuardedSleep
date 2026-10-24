import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, expectNoAxeViolations, pairWeb, waitForBarKind } from "./utils/web";

/** Noise alert (S10) and link alarm (S11) in the page (task 6.2). */

const awayStub = (): void => {
  // The user is in another window: the page has no focus, so a notification is due.
  document.hasFocus = () => false;
  const shown: Array<{ title: string }> = [];
  (window as unknown as { __shown: typeof shown }).__shown = shown;
  class FakeNotification {
    static permission = "granted";
    static requestPermission = () => Promise.resolve("granted");
    constructor(title: string) {
      shown.push({ title });
    }
    close(): void {}
  }
  (window as unknown as { Notification: unknown }).Notification = FakeNotification;
};

test("noise alert: full screen, flashing title, notification, stop sound", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "pulses-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await dual.parentPage.addInitScript(awayStub);
    await pairWeb(dual);
    const parent = dual.parentPage;
    await expect(parent.getByTestId("alert_title")).toBeVisible({ timeout: 90_000 });
    await expect(parent.getByTestId("alert_stop_sound")).toBeVisible();
    await expect(parent.getByTestId("alert_snooze_10")).toBeVisible();
    await expect(parent.getByTestId("alert_listen")).toBeVisible();
    await expect(parent.getByTestId("parent_headline")).toHaveCount(0);
    await expect.poll(() => parent.title()).toBe("Sound detected");
    await expect
      .poll(() =>
        parent.evaluate(() => (window as unknown as { __shown: unknown[] }).__shown.length),
      )
      .toBeGreaterThan(0);
    await expectNoAxeViolations(parent);

    await parent.getByTestId("alert_stop_sound").click();
    await expect(parent.getByTestId("parent_headline")).toBeVisible();
    await expect.poll(() => parent.title()).not.toBe("Sound detected");
  } finally {
    await dual.dispose();
  }
});

// Digital silence, and a 50 ms blip that is shorter than the hold time: neither is an alert.
for (const [fixture, what] of [
  ["silence-30s.wav", "silence"],
  ["blip-50ms.wav", "a short blip"],
] as const) {
  test(`${what} never raises the noise alert`, async () => {
    test.setTimeout(120_000);
    const dual = await launchDualContexts({
      babyFakeWav: join(fixturesDir, fixture),
      parentFakeWav: join(fixturesDir, "silence-30s.wav"),
      permissions: CLIPBOARD_PERMISSIONS,
    });
    try {
      await pairWeb(dual);
      await dual.parentPage.waitForTimeout(12_000);
      await expect(dual.parentPage.getByTestId("alert_title")).toHaveCount(0);
      await expect(dual.parentPage.getByTestId("parent_headline")).toBeVisible();
    } finally {
      await dual.dispose();
    }
  });
}

test("link alarm: baby device stopped, then connection lost", async () => {
  test.setTimeout(240_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await pairWeb(dual);
    const parent = dual.parentPage;

    await dual.babyPage.getByTestId("baby_stop").click();
    await dual.babyPage.getByTestId("dialog_stop_confirm").click();
    await expect(parent.getByTestId("link_alarm_title")).toBeVisible({ timeout: 30_000 });
    await expect(parent.getByTestId("link_alarm_ok")).toBeVisible();
    await expectNoAxeViolations(parent);
    await parent.getByTestId("link_alarm_ok").click();
    await waitForBarKind(parent, "baby_stopped", 10_000);

    // Pair again, then drop the baby page without a goodbye: the core escalates by itself.
    await parent.getByTestId("parent_overflow").click();
    await parent.getByTestId("parent_stop").click();
    await parent.getByTestId("dialog_stop_confirm").click();
    await dual.babyPage.close();
    const baby2 = await dual.baby.newPage();
    dual.babyPage = baby2;
    await pairWeb(dual);
    await baby2.close();

    await expect(parent.getByTestId("link_alarm_title")).toBeVisible({ timeout: 150_000 });
    await expect(parent.getByTestId("link_alarm_ack")).toBeVisible();
    await expect(parent.getByTestId("link_alarm_ack_note")).toBeVisible();
    await expectNoAxeViolations(parent);
    await parent.getByTestId("link_alarm_ack").click();
    await expect(parent.getByTestId("parent_headline")).toBeVisible();
  } finally {
    await dual.dispose();
  }
});
