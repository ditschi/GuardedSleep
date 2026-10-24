import { expect, test, type Page } from "@playwright/test";
import { join } from "node:path";
import { fixturesDir, launchDualContexts } from "./utils/dual-context";
import {
  CLIPBOARD_PERMISSIONS,
  expectNoAxeViolations,
  pasteCode,
  showCode,
  useE2eSignaling,
  waitForBarKind,
} from "./utils/web";

/**
 * Pairing screens of apps/web (tasks 5.1 and 5.2): the baby device shows a QR code and a copyable
 * code, the parent device reads or pastes it. Both pages are separate browsers with a fake
 * microphone and camera.
 */
const silence = join(fixturesDir, "silence-30s.wav");

const cameraRequests = (page: Page): Promise<number> =>
  page.evaluate(() => (window as unknown as { __cameraRequests: number }).__cameraRequests);

test("role screen: two cards, settings, no axe violations", async () => {
  const dual = await launchDualContexts({ babyFakeWav: silence });
  try {
    await dual.babyPage.goto("/");
    await expect(dual.babyPage.getByTestId("role_baby")).toBeVisible();
    await expect(dual.babyPage.getByTestId("role_parent")).toBeVisible();
    await expect(dual.babyPage.getByTestId("role_order_hint")).toBeVisible();
    await expectNoAxeViolations(dual.babyPage);
  } finally {
    await dual.dispose();
  }
});

test("baby pair: QR, countdown, copy code without share, brightness, waiting", async () => {
  test.setTimeout(90_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    const code = await showCode(dual.babyPage);
    const page = dual.babyPage;
    await expect(page.getByTestId("baby_pair_title")).toBeVisible();
    await expect(page.getByTestId("baby_pair_countdown")).toContainText(/[1-5]:\d\d/);
    await expect(page.getByTestId("baby_pair_waiting")).toBeVisible();
    await expect(page.getByTestId("baby_pair_copy_hint")).toBeVisible();
    await expect(page.getByTestId("baby_pair_privacy")).toBeVisible();
    // No share action anywhere on the screen.
    await expect(page.getByRole("button", { name: /share/i })).toHaveCount(0);

    // The code on the clipboard is the v4 invite: 32-byte key, one mailbox entry, a short expiry.
    const invite = JSON.parse(code) as { v: number; k: string; m: string[]; x: number };
    expect(invite.v).toBe(4);
    expect(Buffer.from(invite.k, "base64url")).toHaveLength(32);
    expect(invite.m.length).toBeGreaterThan(0);
    expect(invite.x * 1000).toBeGreaterThan(Date.now());
    expect(invite.x * 1000).toBeLessThanOrEqual(Date.now() + 300_000);

    // The key is drawn, never printed.
    expect(await page.locator("body").innerText()).not.toContain(invite.k);
    expect(await page.content()).not.toContain(invite.k);

    // The code is light on dark at reduced brightness; "Make code brighter" raises it.
    const module = () =>
      page
        .getByTestId("baby_pair_qr")
        .locator("path")
        .evaluate((el) => getComputedStyle(el).fill);
    const dim = await module();
    await page.getByTestId("baby_pair_brighter").click();
    await expect(page.getByTestId("baby_pair_brighter")).toHaveAttribute("aria-pressed", "true");
    expect(await module()).not.toBe(dim);

    await expectNoAxeViolations(page);
  } finally {
    await dual.dispose();
  }
});

test("parent pair: camera only after Allow camera; paste errors per kind", async () => {
  test.setTimeout(90_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    permissions: ["microphone", "camera"],
  });
  try {
    const page = dual.parentPage;
    // Count the camera requests the page makes.
    await page.addInitScript(() => {
      const w = window as unknown as { __cameraRequests: number };
      w.__cameraRequests = 0;
      const md = navigator.mediaDevices;
      const original = md.getUserMedia.bind(md);
      md.getUserMedia = (constraints) => {
        if (constraints?.video) w.__cameraRequests++;
        return original(constraints);
      };
    });
    await page.goto("/");
    await page.getByTestId("role_parent").click();
    await expect(page.getByTestId("parent_pair_title")).toBeVisible();
    await expect(page.getByTestId("parent_pair_camera_why")).toBeVisible();
    expect(await cameraRequests(page)).toBe(0);
    await expectNoAxeViolations(page);

    await page.getByTestId("parent_pair_allow_camera").click();
    await expect(page.locator(".gs-camera__video")).toBeVisible();
    await expect.poll(() => cameraRequests(page)).toBe(1);

    const field = page.getByTestId("parent_pair_paste_field");
    const submit = page.getByTestId("parent_pair_paste_submit");
    await expect(submit).toBeDisabled();

    await field.fill("hello world");
    await submit.click();
    await expect(page.getByTestId("parent_pair_error")).toContainText("isn't a GuardedSleep code");

    await field.fill('{"v":4,"k":"AAAA');
    await submit.click();
    await expect(page.getByTestId("parent_pair_error")).toContainText("incomplete");

    const expired = JSON.stringify({
      v: 4,
      k: "A".repeat(43),
      m: ["l:127.0.0.1:1"],
      x: Math.floor(Date.now() / 1000) - 60,
    });
    await field.fill(expired);
    await submit.click();
    await expect(page.getByTestId("parent_pair_error")).toContainText("has expired");
    await expectNoAxeViolations(page);

    await page.getByTestId("parent_pair_retry").click();
    await expect(page.getByTestId("parent_pair_error")).toHaveCount(0);
  } finally {
    await dual.dispose();
  }
});

test("pairing by pasting the code connects both devices and leaves the code screen", async () => {
  test.setTimeout(120_000);
  const dual = await launchDualContexts({
    babyFakeWav: silence,
    parentFakeWav: silence,
    permissions: CLIPBOARD_PERMISSIONS,
  });
  try {
    const code = await showCode(dual.babyPage);
    await pasteCode(dual.parentPage, code);

    await dual.babyPage.waitForURL(/\/baby$/, { timeout: 60_000 });
    await dual.parentPage.waitForURL(/\/parent$/, { timeout: 60_000 });
    await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
      "data-state",
      "connected",
      { timeout: 60_000 },
    );
    await waitForBarKind(dual.parentPage, "connected");
    await expect(dual.babyPage.getByTestId("baby_pair_qr")).toHaveCount(0);
  } finally {
    await dual.dispose();
  }
});

test("a pending baby pairing whose microphone is refused names the microphone", async () => {
  test.setTimeout(60_000);
  const dual = await launchDualContexts({ babyFakeWav: silence });
  try {
    // The fake-UI flag grants every prompt, so the refusal is simulated at the API.
    await dual.babyPage.addInitScript(() => {
      navigator.mediaDevices.getUserMedia = () =>
        Promise.reject(new DOMException("denied", "NotAllowedError"));
    });
    await useE2eSignaling(dual.babyPage);
    await dual.babyPage.getByTestId("role_baby").click();
    const error = dual.babyPage.getByTestId("baby_pair_error");
    await expect(error).toBeVisible({ timeout: 30_000 });
    await expect(error).toContainText(/microphone/i);
    await expect(error).not.toContainText(/reach|find the baby/i);
    await expect(dual.babyPage.getByTestId("baby_pair_new_code")).toBeVisible();
  } finally {
    await dual.dispose();
  }
});
