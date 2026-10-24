import { expect, test, type Page } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import { checkScreen, fitProblems, NARROW_VIEWPORT } from "./utils/layout";
import { rtcTrackerStub, closePeerConnections } from "./utils/rtc";
import { CLIPBOARD_PERMISSIONS, pairWeb, pasteCode, showCode } from "./utils/web";

/**
 * Every screen of the web client is checked twice (task 8.2): axe for accessibility, and a
 * text-overflow check at 320 px width with text at 200 % (`utils/layout.ts`: no sideways page
 * scroll, nothing cut off, nothing pushed out of reach). Dialogs and sheets count as screens.
 * The role, pairing, monitoring, alert, link alarm, night and settings screens of both devices
 * are walked through in a few sessions, because most of them need a live pairing.
 */

const silence = join(fixturesDir, "silence-30s.wav");
const pulses = join(fixturesDir, "pulses-30s.wav");

async function openAndCheck(page: Page, openId: string, shownId: string, screen: string) {
  await page.getByTestId(openId).click();
  await expect(page.getByTestId(shownId)).toBeVisible();
  await checkScreen(page, screen);
}

test("the overflow check finds what it is meant to find", async () => {
  const dual = await launchDualContexts({ babyFakeWav: silence });
  const page = dual.babyPage;
  try {
    await page.goto("/");
    await page.setViewportSize(NARROW_VIEWPORT);
    expect(await fitProblems(page), "the role screen fits").toEqual([]);

    await page.evaluate(() => {
      const wide = document.createElement("p");
      wide.textContent = "far too wide for a phone";
      wide.style.cssText = "width:600px;white-space:nowrap";
      const clipped = document.createElement("p");
      clipped.textContent = "a long sentence that is cut off by its own box";
      clipped.style.cssText = "width:40px;overflow:hidden;white-space:nowrap";
      const fixed = document.createElement("p");
      fixed.textContent = "stuck below the screen";
      fixed.style.cssText = "position:fixed;top:900px;left:0";
      document.querySelector("main")?.append(wide, clipped, fixed);
    });
    const problems = (await fitProblems(page)).join("\n");
    expect(problems).toContain("page scrolls sideways");
    expect(problems).toContain("far too wide");
    expect(problems).toContain("clips its text sideways");
    expect(problems).toContain("lies outside the screen in a fixed layer");
  } finally {
    await dual.dispose();
  }
});

test("start, settings, pairing, monitoring and night screens of both devices", async () => {
  test.setTimeout(240_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const { babyPage: baby, parentPage: parent } = dual;
  try {
    // ---- start screen and settings (before any pairing)
    await baby.goto("/");
    await expect(baby.getByTestId("role_title")).toBeVisible();
    await checkScreen(baby, "role");

    await baby.getByTestId("home_settings").click();
    await expect(baby.getByTestId("settings_title")).toBeVisible();
    await checkScreen(baby, "settings");
    await baby.getByTestId("settings_advanced_toggle").click();
    await expect(baby.getByTestId("dialog_advanced")).toBeVisible();
    await checkScreen(baby, "settings: advanced addresses warning");
    await baby.getByTestId("dialog_advanced_confirm").click();
    await expect(baby.getByTestId("settings_url_input")).toBeVisible();
    await checkScreen(baby, "settings: advanced addresses");
    await baby.getByTestId("settings_back").click();

    // ---- pairing screens
    const code = await showCode(baby);
    await expect(baby.getByTestId("baby_pair_qr")).toBeVisible();
    await checkScreen(baby, "baby pairing");

    await parent.goto("/");
    await parent.getByTestId("role_parent").click();
    await expect(parent.getByTestId("parent_pair_title")).toBeVisible();
    await checkScreen(parent, "parent pairing");
    await parent.getByTestId("parent_pair_paste_field").fill("hello world");
    await parent.getByTestId("parent_pair_paste_submit").click();
    await expect(parent.getByTestId("parent_pair_error")).toBeVisible();
    await checkScreen(parent, "parent pairing: not our code");

    await pasteCode(parent, code);
    await expect(parent.getByTestId("parent_ambient_bar")).toHaveAttribute(
      "data-kind",
      "connected",
      { timeout: 60_000 },
    );
    await expect(baby.getByTestId("baby_status")).toHaveAttribute("data-state", "connected", {
      timeout: 60_000,
    });

    // ---- baby monitoring
    await checkScreen(baby, "baby monitoring");
    await openAndCheck(baby, "baby_stop", "dialog_stop", "baby: stop dialog");
    await baby.getByTestId("dialog_stop_cancel").click();
    await openAndCheck(baby, "baby_settings", "settings_sensitivity", "baby: settings");
    await baby.getByTestId("settings_back").click();
    await openAndCheck(baby, "baby_night_open", "night_screen", "baby: night screen");
    await baby.getByTestId("night_screen_tap").click();
    await expect(baby.getByTestId("night_screen_status")).toBeVisible();
    await expect(baby.getByTestId("night_screen")).toHaveCSS("opacity", "1");
    await checkScreen(baby, "baby: night screen with status");
    await baby.getByTestId("night_screen_tap").click();

    // ---- parent monitoring
    await expect(parent.getByTestId("parent_headline")).toBeVisible();
    await checkScreen(parent, "parent monitoring");
    await openAndCheck(parent, "parent_snooze", "sheet_snooze_10", "parent: snooze sheet");
    await parent.getByTestId("sheet_snooze_10").click();
    await expect(parent.getByTestId("parent_snooze_chip")).toBeVisible();
    await checkScreen(parent, "parent monitoring while snoozed");
    await openAndCheck(
      parent,
      "parent_snooze",
      "sheet_snooze_end",
      "parent: snooze sheet (active)",
    );
    await parent.getByTestId("sheet_snooze_end").click();
    await openAndCheck(parent, "parent_probe", "sheet_probe_run", "parent: test alert sheet");
    await parent.keyboard.press("Escape");
    await expect(parent.getByTestId("sheet_probe_run")).toBeHidden();
    await openAndCheck(parent, "parent_overflow", "parent_settings", "parent: more sheet");
    await parent.getByTestId("parent_settings").click();
    await expect(parent.getByTestId("settings_sensitivity_note")).toBeVisible();
    await checkScreen(parent, "parent: settings");
    await parent.getByTestId("settings_forget").click();
    await expect(parent.getByTestId("dialog_forget")).toBeVisible();
    await checkScreen(parent, "parent: forget dialog");
    await parent.getByTestId("dialog_forget_cancel").click();
    await parent.getByTestId("settings_back").click();
    await openAndCheck(parent, "parent_night_open", "night_screen", "parent: night screen");
    await parent.getByTestId("night_screen_tap").click();
    await expect(parent.getByTestId("night_screen")).toHaveCSS("opacity", "1");
    await checkScreen(parent, "parent: night screen with status");
    await parent.keyboard.press("Escape");

    // ---- the stop dialog of the parent, then the baby learns about it
    await parent.getByTestId("parent_overflow").click();
    await parent.getByTestId("parent_stop").click();
    await expect(parent.getByTestId("dialog_stop")).toBeVisible();
    await checkScreen(parent, "parent: stop dialog");
  } finally {
    await dual.dispose();
  }
});

test("noise alert screen", async () => {
  test.setTimeout(180_000);
  const dual = await launchDualContexts({
    babyFakeWav: pulses,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    await pairWeb(dual);
    await expect(dual.parentPage.getByTestId("alert_title")).toBeVisible({ timeout: 90_000 });
    await checkScreen(dual.parentPage, "parent: noise alert");
  } finally {
    await dual.dispose();
  }
});

test("link alarm screens: baby device stopped, and connection lost", async () => {
  test.setTimeout(300_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  const parent = dual.parentPage;
  try {
    await dual.parent.addInitScript(rtcTrackerStub);
    await pairWeb(dual);

    // The baby device stops on purpose.
    await dual.babyPage.getByTestId("baby_stop").click();
    await dual.babyPage.getByTestId("dialog_stop_confirm").click();
    await expect(parent.getByTestId("link_alarm_title")).toBeVisible({ timeout: 30_000 });
    await checkScreen(parent, "parent: link alarm, baby device stopped");
    await parent.getByTestId("link_alarm_stop").click();
    await expect(parent.getByTestId("role_title")).toBeVisible();

    // Pair again and lose the connection: the parent goes offline.
    await pairWeb(dual);
    await dual.parent.setOffline(true);
    await closePeerConnections(parent);
    await expect(parent.getByTestId("parent_link_alarm")).toBeVisible({ timeout: 120_000 });
    await checkScreen(parent, "parent: link alarm, connection lost");
    await dual.parent.setOffline(false);
  } finally {
    await dual.dispose();
  }
});
