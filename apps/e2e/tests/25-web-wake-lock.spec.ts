import { expect, test } from "@playwright/test";
import { join } from "node:path";
import {
  noWakeLockStub,
  revokeWakeLocks,
  wakeLockCounts,
  wakeLockStub,
} from "./utils/browser-stubs";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { CLIPBOARD_PERMISSIONS, pairWeb } from "./utils/web";

/**
 * Screen Wake Lock while monitoring (task 7.1) with a stubbed `navigator.wakeLock`: taken on both
 * devices once they monitor, a warning with a retry when the system takes it away, released on stop.
 */
test("wake lock: held while monitoring, warns when revoked, retried, released on stop", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const baby = dual.babyPage;
  const parent = dual.parentPage;
  try {
    await dual.baby.addInitScript(wakeLockStub);
    await dual.parent.addInitScript(wakeLockStub);
    await pairWeb(dual);

    // Both devices hold exactly one lock and show no warning.
    await expect.poll(() => wakeLockCounts(baby)).toMatchObject({ active: 1 });
    await expect.poll(() => wakeLockCounts(parent)).toMatchObject({ active: 1 });
    await expect(baby.getByTestId("baby_wake_lock")).toBeVisible();
    await expect(baby.getByTestId("wake_lock_warning")).toHaveCount(0);
    await expect(parent.getByTestId("parent_banner_browser_keep_open_lost")).toHaveCount(0);

    // The system takes the parent's lock away: a warning with a button, and the lock comes back.
    await revokeWakeLocks(parent);
    // On the parent it is the keep-open banner of the stack, turned into a warning.
    await expect(parent.getByTestId("parent_banner_browser_keep_open_lost")).toBeVisible();
    await expect.poll(() => wakeLockCounts(parent)).toMatchObject({ active: 0 });
    await parent.getByTestId("parent_banner_browser_keep_open_lost_action").click();
    await expect(parent.getByTestId("parent_banner_browser_keep_open_lost")).toHaveCount(0);
    await expect.poll(() => wakeLockCounts(parent)).toMatchObject({ active: 1 });

    // The baby device's loss reaches the parent as a unit warning notice.
    await revokeWakeLocks(baby);
    await expect(baby.getByTestId("wake_lock_warning")).toBeVisible();
    await expect(baby.getByTestId("baby_wake_lock")).toHaveCount(0);
    await expect(parent.getByTestId("parent_banner_unit_warning_wake_lock_off")).toBeVisible({
      timeout: 30_000,
    });

    // Stopping gives the lock back.
    await parent.getByTestId("parent_overflow").click();
    await parent.getByTestId("parent_stop").click();
    await parent.getByTestId("dialog_stop_confirm").click();
    await expect.poll(() => wakeLockCounts(parent)).toMatchObject({ active: 0 });
  } finally {
    await dual.dispose();
  }
});

test("wake lock: a browser without the API says so plainly", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: join(fixturesDir, "silence-30s.wav"),
    parentFakeWav: join(fixturesDir, "silence-30s.wav"),
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await dual.baby.addInitScript(noWakeLockStub);
    await dual.parent.addInitScript(noWakeLockStub);
    await pairWeb(dual);
    await expect(dual.babyPage.getByTestId("wake_lock_warning")).toHaveAttribute(
      "data-state",
      "unsupported",
    );
    await expect(dual.babyPage.getByTestId("wake_lock_retry")).toHaveCount(0);
    await expect(
      dual.parentPage.getByTestId("parent_banner_browser_keep_open_unsupported"),
    ).toBeVisible();
    await expect(
      dual.parentPage.getByTestId("parent_banner_browser_keep_open_unsupported_action"),
    ).toHaveCount(0);
  } finally {
    await dual.dispose();
  }
});
