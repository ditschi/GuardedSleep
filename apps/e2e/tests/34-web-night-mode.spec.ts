import { expect, test } from "@playwright/test";
import { expectNoAxeViolations } from "./utils/web";

/**
 * Night mode (`redesign-calm-ui` 6.1, `web-client` "Accessible navigation"): the choice is
 * remembered on the device and the Ember scheme is on the first painted frame, without a bright
 * flash. The status colours are covered by Vitest (the stylesheet rules) and the Android tests.
 */
test("night mode: set in Settings, Ember before the first paint after a reload, axe clean", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator("html")).toHaveAttribute("data-scheme", "dusk");
  await page.getByTestId("home_settings").click();
  await page.getByTestId("settings_night_on").click();
  await expect(page.locator("html")).toHaveAttribute("data-scheme", "ember");
  await expectNoAxeViolations(page);
  await page.getByTestId("settings_night_auto").click();
  await expect(page.getByTestId("settings_night_window")).toBeVisible();
  await expect(page.getByTestId("settings_night_status")).toContainText("turns");
  await page.getByTestId("settings_night_on").click();

  // Record every data-scheme value from the very first moment of the next page load.
  await page.addInitScript(() => {
    const seen: string[] = [];
    (window as unknown as { __schemes: string[] }).__schemes = seen;
    new MutationObserver(() => {
      seen.push(document.documentElement.dataset.scheme ?? "");
    }).observe(document, { attributes: true, subtree: true, attributeFilter: ["data-scheme"] });
  });
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-scheme", "ember");
  const schemes = await page.evaluate(
    () => (window as unknown as { __schemes: string[] }).__schemes,
  );
  // The boot script set Ember first; the page never painted Dusk or Day in between.
  expect(schemes[0]).toBe("ember");
  expect(schemes).not.toContain("dusk");
  expect(schemes).not.toContain("day");

  // Off again: the next load is Dusk.
  await page.getByTestId("home_settings").click();
  await page.getByTestId("settings_night_off").click();
  await expect(page.locator("html")).toHaveAttribute("data-scheme", "dusk");
});
