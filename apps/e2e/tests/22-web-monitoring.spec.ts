import { expect, test } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, expectNoAxeViolations, pairWeb, waitForBarKind } from "./utils/web";

/**
 * Monitoring screens of apps/web (task 6.1): baby status, mic level and keep-awake advice; parent
 * ambient bar, headline, meter, chips, Listen, snooze, test alert and Stop inside More.
 */
test("monitoring screens with fake media: content, a11y, listen, stop", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const baby = dual.babyPage;
  const parent = dual.parentPage;
  try {
    await pairWeb(dual);

    // ---- baby device
    await expect(baby.getByTestId("baby_monitoring_line")).toBeVisible();
    await expect(baby.getByTestId("baby_mic_level")).toBeVisible();
    await expect(baby.getByTestId("baby_silence")).toBeVisible();
    await expect(baby.getByTestId("baby_keep_awake")).toBeVisible();
    await expect(baby.getByTestId("baby_stop")).toBeVisible();
    await expect(baby.locator("html")).toHaveAttribute("data-theme", "dim");
    await expectNoAxeViolations(baby);

    // ---- parent device
    await expect(parent.getByTestId("parent_headline")).toContainText(/All quiet since|Monitoring/);
    await expect(parent.getByTestId("parent_meter")).toBeVisible();
    await expect(parent.getByTestId("parent_keep_open_banner")).toBeVisible();
    await expect(parent.getByTestId("parent_overflow")).toBeVisible();
    // The browser baby device reports its silence as unknown, in grey, and cannot be silenced.
    await expect(parent.getByTestId("parent_silence_chip")).toContainText(
      "turn it on at the baby device",
      { timeout: 30_000 },
    );
    await expect(parent.getByTestId("parent_silence_chip")).toHaveAttribute("data-tone", "neutral");
    await expect(parent.getByTestId("sheet_silence_apply")).toHaveCount(0);

    // Listen is at least 72 px tall and 48 px wide targets everywhere.
    const listen = parent.getByTestId("parent_listen");
    await expect(listen).toBeEnabled();
    const box = await listen.boundingBox();
    expect(box?.height ?? 0).toBeGreaterThanOrEqual(72);
    for (const id of ["parent_snooze", "parent_probe", "parent_overflow"]) {
      const target = await parent.getByTestId(id).boundingBox();
      expect(target?.height ?? 0, id).toBeGreaterThanOrEqual(48);
      expect(target?.width ?? 0, id).toBeGreaterThanOrEqual(48);
    }
    await expectNoAxeViolations(parent);

    // Listening (keyboard: switches on until the next activation).
    await listen.focus();
    await parent.keyboard.press("Enter");
    await expect(listen).toHaveAttribute("data-listen", "locked");
    await expect(listen).toContainText(/Listening/);
    await parent.keyboard.press("Enter");
    await expect(listen).toHaveAttribute("data-listen", "off");

    // Snooze through the sheet and end it again.
    await parent.getByTestId("parent_snooze").click();
    await parent.getByTestId("sheet_snooze_10").click();
    await expect(parent.getByTestId("parent_snooze_chip")).toBeVisible();
    await parent.getByTestId("parent_snooze").click();
    await expectNoAxeViolations(parent);
    await parent.getByTestId("sheet_snooze_end").click();
    await expect(parent.getByTestId("parent_snooze_chip")).toHaveCount(0);

    // Stop is inside More and asks first.
    await parent.getByTestId("parent_overflow").click();
    await parent.getByTestId("parent_stop").click();
    await parent.getByTestId("dialog_stop_confirm").click();
    await expect(parent.getByTestId("role_title")).toBeVisible();

    // The baby device learns that the parent stopped.
    await expect(baby.getByTestId("baby_status")).toHaveAttribute("data-state", "peer_stopped", {
      timeout: 30_000,
    });
    await baby.getByTestId("baby_stop").click();
    await baby.getByTestId("dialog_stop_confirm").click();
    await expect(baby.getByTestId("role_title")).toBeVisible();
    await waitForBarKind(parent, /./, 1).catch(() => undefined);
  } finally {
    await dual.dispose();
  }
});
