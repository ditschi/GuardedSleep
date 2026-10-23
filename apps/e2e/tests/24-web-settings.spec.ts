import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, expectNoAxeViolations, pairWeb } from "./utils/web";

/** Settings screen (task 6.3). */
test("settings: theme survives a reload, language, addresses, sensitivity, forget", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const page = dual.babyPage;
  try {
    await page.goto("/");
    await page.getByTestId("home_settings").click();
    await expect(page.getByTestId("settings_title")).toBeFocused();
    await expectNoAxeViolations(page);

    // Theme: applied now and again before the first paint after a reload.
    await page.getByTestId("settings_theme_light").click();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await expectNoAxeViolations(page);
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
    await page.getByTestId("settings_theme_dark").click();

    // Language.
    await page.getByTestId("settings_language").selectOption("de");
    await expect(page.locator("html")).toHaveAttribute("lang", "de");
    await page.getByTestId("settings_language").selectOption("");
    await expect(page.locator("html")).toHaveAttribute("lang", "en");

    // Addresses sit behind the warning; a wrong address is refused.
    await expect(page.getByTestId("settings_url_input")).toHaveCount(0);
    await page.getByTestId("settings_advanced_toggle").click();
    await page.getByTestId("dialog_advanced_cancel").click();
    await expect(page.getByTestId("settings_advanced_toggle")).not.toBeChecked();
    await page.getByTestId("settings_advanced_toggle").click();
    await page.getByTestId("dialog_advanced_confirm").click();
    await page.getByTestId("settings_url_input").fill("http://nope.test");
    await page.getByTestId("settings_url_add").click();
    await expect(page.getByTestId("settings_url_error")).toBeVisible();
    await page.getByTestId("settings_url_input").fill("wss://relay.example.test");
    await page.getByTestId("settings_url_add").click();
    await expect(page.getByTestId("settings_url_list")).toContainText("wss://relay.example.test");
    await expectNoAxeViolations(page);
    await page.getByTestId("settings_url_remove_0").click();
    await expect(page.getByTestId("settings_url_list")).not.toContainText("relay.example.test");

    // Pair, then sensitivity (baby device only) and forgetting the pairing.
    await pairWeb(dual);
    await page.getByTestId("baby_settings").click();
    await expect(page.getByTestId("settings_sensitivity")).toBeVisible();
    await page.getByTestId("settings_sensitivity").fill("100");
    await expectNoAxeViolations(page);
    await page.getByTestId("settings_back").click();
    await expect(page.getByTestId("baby_status")).toBeVisible();

    await dual.parentPage.getByTestId("parent_overflow").click();
    await dual.parentPage.getByTestId("parent_settings").click();
    await expect(dual.parentPage.getByTestId("settings_sensitivity_note")).toBeVisible();
    await dual.parentPage.getByTestId("settings_forget").click();
    await dual.parentPage.getByTestId("dialog_forget_confirm").click();
    await expect(dual.parentPage.getByTestId("role_title")).toBeVisible();
  } finally {
    await dual.dispose();
  }
});
