import { expect, test, type Page } from "@playwright/test";
import { join } from "node:path";
import {
  pageGuardStub,
  setHidden,
  shownNotifications,
  skewClock,
  wakeLockStub,
} from "./utils/browser-stubs";
import { fixturesDir, launchDualContexts, type DualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, expectNoAxeViolations, pairWeb } from "./utils/web";

/**
 * Background-loss reporting (task 7.3): a notification when the monitoring page is hidden, the
 * leave-page confirmation, the away report on return, the baby device's warnings reaching the
 * parent, and the resume-from-sleep warning (a clock skew stands in for the sleeping laptop).
 */

async function launch(): Promise<DualContexts> {
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  for (const ctx of [dual.baby, dual.parent]) {
    // A held wake lock keeps its own warnings out of these tests.
    await ctx.addInitScript(wakeLockStub);
    await ctx.addInitScript(pageGuardStub);
  }
  return dual;
}

const connected = async (page: Page): Promise<void> => {
  await expect(page.getByTestId("parent_ambient_bar")).toHaveAttribute("data-kind", "connected", {
    timeout: 60_000,
  });
};

test("hidden parent page: notification, away report on return, no alarm for a short stay", async () => {
  test.setTimeout(180_000);
  const dual = await launch();
  const parent = dual.parentPage;
  try {
    await pairWeb(dual);
    await expect(parent.getByTestId("away_notice")).toHaveCount(0);

    // Another tab comes to the front: the page says so in a system notification.
    await setHidden(parent, true);
    await expect
      .poll(async () => (await shownNotifications(parent)).map((n) => n.tag))
      .toContain("guardedsleep-hidden");
    const note = (await shownNotifications(parent)).find((n) => n.tag === "guardedsleep-hidden");
    expect(note?.title).toBe("GuardedSleep may stop monitoring");
    expect(note?.closed).toBe(false);

    // Half a minute later the page returns: the notification closes and the report names the time.
    await skewClock(parent, 30_000);
    await setHidden(parent, false);
    await expect
      .poll(
        async () =>
          (await shownNotifications(parent)).find((n) => n.tag === "guardedsleep-hidden")?.closed,
      )
      .toBe(true);
    const away = parent.getByTestId("away_notice");
    await expect(away).toHaveAttribute("data-kind", "background");
    await expect(away).toContainText("in the background for");
    await expect(away).toContainText("Alerts may have been missed");
    await expectNoAxeViolations(parent);
    await away.getByRole("button", { name: "Dismiss" }).click();
    await expect(away).toHaveCount(0);

    // A glance of a few seconds is not reported.
    await setHidden(parent, true);
    await setHidden(parent, false);
    await expect(parent.getByTestId("away_notice")).toHaveCount(0);
    await connected(parent);
  } finally {
    await dual.dispose();
  }
});

test("hidden baby page: the parent sees the warning, the baby sees the report on return", async () => {
  test.setTimeout(180_000);
  const dual = await launch();
  const { babyPage: baby, parentPage: parent } = dual;
  try {
    await pairWeb(dual);
    await setHidden(baby, true);
    await expect(parent.getByTestId("parent_unit_warning_page_hidden")).toBeVisible({
      timeout: 30_000,
    });
    await expect
      .poll(async () => (await shownNotifications(baby)).map((n) => n.tag))
      .toContain("guardedsleep-hidden");

    await skewClock(baby, 30_000);
    await setHidden(baby, false);
    await expect(baby.getByTestId("away_notice")).toHaveAttribute("data-kind", "background");
    await expect(parent.getByTestId("parent_unit_warning_page_hidden")).toHaveCount(0, {
      timeout: 30_000,
    });
  } finally {
    await dual.dispose();
  }
});

test("resume from sleep: both pages warn, the parent is told about the baby device", async () => {
  test.setTimeout(180_000);
  const dual = await launch();
  const { babyPage: baby, parentPage: parent } = dual;
  try {
    await pairWeb(dual);

    // The baby laptop sleeps for 20 minutes; its next timer finds the clock far ahead.
    await skewClock(baby, 20 * 60_000);
    const away = baby.getByTestId("away_notice");
    await expect(away).toHaveAttribute("data-kind", "sleep", { timeout: 15_000 });
    await expect(away).toContainText("device was asleep for 20 min");
    await expect(parent.getByTestId("parent_unit_warning_resumed_from_sleep")).toBeVisible({
      timeout: 30_000,
    });

    // The parent laptop sleeps too: the core gets the time and the link recovers or escalates by
    // itself; the page only says it was asleep.
    await skewClock(parent, 20 * 60_000);
    await expect(parent.getByTestId("away_notice")).toHaveAttribute("data-kind", "sleep", {
      timeout: 15_000,
    });
  } finally {
    await dual.dispose();
  }
});

test("leaving the page while monitoring asks for a confirmation", async () => {
  test.setTimeout(120_000);
  const dual = await launch();
  const parent = dual.parentPage;
  try {
    await pairWeb(dual);
    const dialogs: string[] = [];
    parent.on("dialog", (dialog) => {
      dialogs.push(dialog.type());
      void dialog.dismiss();
    });
    // The event itself is cancelable only while monitoring.
    const prevented = await parent.evaluate(() => {
      const event = new Event("beforeunload", { cancelable: true });
      window.dispatchEvent(event);
      return event.defaultPrevented;
    });
    expect(prevented).toBe(true);
    // A real navigation away raises the browser's own confirmation; dismissing it stays.
    await parent.goto("about:blank").catch(() => undefined);
    expect(dialogs).toContain("beforeunload");
    await expect(parent.getByTestId("parent_headline")).toBeVisible();

    // After stopping, nothing holds the page back.
    await parent.getByTestId("parent_overflow").click();
    await parent.getByTestId("parent_stop").click();
    await parent.getByTestId("dialog_stop_confirm").click();
    await expect(parent.getByTestId("role_title")).toBeVisible();
    const preventedAfterStop = await parent.evaluate(() => {
      const event = new Event("beforeunload", { cancelable: true });
      window.dispatchEvent(event);
      return event.defaultPrevented;
    });
    expect(preventedAfterStop).toBe(false);
  } finally {
    await dual.dispose();
  }
});
