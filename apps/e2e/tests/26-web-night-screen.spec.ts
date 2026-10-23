import { expect, test, type Page } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, expectNoAxeViolations, pairWeb } from "./utils/web";

/** Night screen on both devices (task 7.2). */

const dotAnimation = (page: Page): Promise<string> =>
  page
    .locator(".gs-night__dot")
    .evaluate((el) => getComputedStyle(el).animationName)
    .catch(() => "");

async function expectBlackFullScreen(page: Page): Promise<void> {
  const box = await page.getByTestId("night_screen").boundingBox();
  const size = page.viewportSize();
  expect(box?.width).toBe(size?.width);
  expect(box?.height).toBe(size?.height);
  const colors = await page
    .getByTestId("night_screen")
    .evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(colors).toBe("rgb(0, 0, 0)");
}

test("night screen: dark and full screen, first tap shows status, second tap closes", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await pairWeb(dual);
    for (const [page, openId] of [
      [dual.babyPage, "baby_night_open"],
      [dual.parentPage, "parent_night_open"],
    ] as const) {
      await page.getByTestId(openId).click();
      await expect(page.getByTestId("night_screen")).toBeVisible();
      await expectBlackFullScreen(page);
      await expect(page.getByTestId("night_screen_status")).toHaveCount(0);
      // The dot drifts (an animation is running) while motion is allowed.
      expect(await dotAnimation(page)).toBe("gs-night-drift");
      await expectNoAxeViolations(page);

      // First tap: the status, with the way out. Second tap: back to the monitor.
      await page.getByTestId("night_screen_tap").click();
      await expect(page.getByTestId("night_screen_status")).toContainText("Still monitoring");
      await expect(page.getByTestId("night_screen_status")).toContainText("Tap again to leave");
      await expectNoAxeViolations(page);
      await page.getByTestId("night_screen_tap").click();
      await expect(page.getByTestId("night_screen")).toBeHidden();
      await expect(page.getByTestId(openId)).toBeFocused();
    }

    // The status goes away by itself and Escape leaves the night screen.
    await dual.parentPage.getByTestId("parent_night_open").click();
    await dual.parentPage.getByTestId("night_screen_tap").click();
    await expect(dual.parentPage.getByTestId("night_screen_status")).toBeVisible();
    await expect(dual.parentPage.getByTestId("night_screen_status")).toHaveCount(0, {
      timeout: 15_000,
    });
    await dual.parentPage.keyboard.press("Escape");
    await expect(dual.parentPage.getByTestId("night_screen")).toBeHidden();
  } finally {
    await dual.dispose();
  }
});

test("night screen: the dot stands still when the user prefers reduced motion", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await dual.babyPage.emulateMedia({ reducedMotion: "reduce" });
    await dual.parentPage.emulateMedia({ reducedMotion: "reduce" });
    await pairWeb(dual);
    for (const [page, openId] of [
      [dual.babyPage, "baby_night_open"],
      [dual.parentPage, "parent_night_open"],
    ] as const) {
      await page.getByTestId(openId).click();
      await expect(page.getByTestId("night_screen")).toBeVisible();
      expect(await dotAnimation(page)).toBe("none");
      await page.keyboard.press("Escape");
    }
  } finally {
    await dual.dispose();
  }
});

test("night screen: a parent alert replaces it at once", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await pairWeb(dual);
    const parent = dual.parentPage;
    await parent.getByTestId("parent_night_open").click();
    await expect(parent.getByTestId("night_screen")).toBeVisible();
    // The baby device stops: the link alarm takes over the whole screen.
    await dual.babyPage.getByTestId("baby_stop").click();
    await dual.babyPage.getByTestId("dialog_stop_confirm").click();
    await expect(parent.getByTestId("link_alarm_title")).toBeVisible({ timeout: 30_000 });
    await expect(parent.getByTestId("night_screen")).toHaveCount(0);
    await parent.getByTestId("link_alarm_ok").click();
    await expect(parent.getByTestId("parent_headline")).toBeVisible();
    await expect(parent.getByTestId("night_screen")).toBeHidden();
  } finally {
    await dual.dispose();
  }
});
