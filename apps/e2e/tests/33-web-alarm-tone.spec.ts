import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, expectNoAxeViolations, pairWeb } from "./utils/web";

/**
 * Connection alarm tone (`add-connection-alarm-tone` 3.2): the parent picks a tone in
 * Settings > Alerts, the choice is stored next to the theme setting, and a preview plays after the
 * click that unlocks the sound (the sound gate). Select by test id, never by copy.
 */
test("alarm tone: pick a tone, then preview and test it after the sound gate", async () => {
  test.setTimeout(180_000);
  const silence = join(fixturesDir, "silence-30s.wav");
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const page = dual.parentPage;
  try {
    await pairWeb(dual);
    await page.getByTestId("parent_overflow").click();
    await page.getByTestId("parent_settings").click();

    // The row shows the default; the baby device has no such row.
    await expect(page.getByTestId("settings_alarm_tone_name")).toBeVisible();
    await page.getByTestId("settings_alarm_tone").click();
    await expect(page.getByTestId("alarm_tone_picker")).toBeVisible();
    await expect(page.getByTestId("alarm_tone_title")).toBeFocused();
    await expect(page.getByTestId("alarm_tone_classic")).toBeChecked();
    await expect(page.getByTestId("alarm_tone_tag_low")).toBeVisible();
    await expect(page.getByTestId("alarm_tone_hint")).toBeVisible();
    await expectNoAxeViolations(page);

    // Choosing: stored next to the theme, and the row shows it after going back.
    await page.getByTestId("alarm_tone_beacon").check();
    await expect(page.getByTestId("alarm_tone_beacon")).toBeChecked();
    expect(await page.evaluate(() => localStorage.getItem("guardedsleep.alarmTone"))).toBe(
      "beacon",
    );

    // Preview: the click is the gesture that unlocks the sound; the button turns into Stop.
    const preview = page.getByTestId("alarm_tone_preview_beacon");
    await expect(preview).toHaveAttribute("aria-pressed", "false");
    await preview.click();
    await expect(preview).toHaveAttribute("aria-pressed", "true");
    await preview.click();
    await expect(preview).toHaveAttribute("aria-pressed", "false");

    // Test: full volume, with a Stop that ends it.
    await expect(page.getByTestId("alarm_tone_test_warning")).toBeVisible();
    await page.getByTestId("alarm_tone_test").click();
    await expect(page.getByTestId("alarm_tone_test_stop")).toBeVisible();
    await page.getByTestId("alarm_tone_test_stop").click();
    await expect(page.getByTestId("alarm_tone_test")).toBeVisible();

    await page.getByTestId("alarm_tone_back").click();
    await expect(page.getByTestId("settings_alarm_tone")).toBeVisible();
  } finally {
    await dual.dispose();
  }
});
