import AxeBuilder from "@axe-core/playwright";
import { expect, type Page } from "@playwright/test";
import type { DualContexts } from "./dual-context";

/*
 * Helpers for the apps/web screens. They select by test id (alpha-ux-flows.md section 4), never
 * by copy, and move through the pages client-side: the pairing key lives in page memory only, so
 * a full navigation would forget it.
 */

export const CLIPBOARD_PERMISSIONS = ["microphone", "clipboard-read", "clipboard-write"];

const SIGNALING = process.env.E2E_SIGNALING_URL ?? "ws://127.0.0.1:18787";

/**
 * Points the page at the e2e signaling server through the settings screen (advanced addresses).
 * The dev default is 8787, which a developer's own stack may be using. The addresses live in
 * page memory, so the baby device does this once per page load; the parent device takes the
 * addresses from the code.
 */
export async function useE2eSignaling(page: Page, url: string = SIGNALING): Promise<void> {
  await page.goto("/");
  await page.getByTestId("home_settings").click();
  await page.getByTestId("settings_advanced_toggle").click();
  await page.getByTestId("dialog_advanced_confirm").click();
  await page.getByTestId("settings_url_input").fill(url);
  await page.getByTestId("settings_url_add").click();
  await page.getByTestId("settings_back").click();
}

/** Baby device: opens the pairing screen and returns the code from the clipboard. */
export async function showCode(page: Page, signalingUrl?: string): Promise<string> {
  await useE2eSignaling(page, signalingUrl);
  await page.getByTestId("role_baby").click();
  await expect(page.getByTestId("baby_pair_qr")).toBeVisible({ timeout: 20_000 });
  await page.getByTestId("baby_pair_copy").click();
  await expect(page.getByTestId("baby_pair_copy")).toContainText(/\S/);
  return page.evaluate(() => navigator.clipboard.readText());
}

/** Parent device: pastes the code on the pairing screen and connects. */
export async function pasteCode(page: Page, code: string): Promise<void> {
  await page.goto("/");
  await page.getByTestId("role_parent").click();
  await page.getByTestId("parent_pair_paste_field").fill(code);
  await page.getByTestId("parent_pair_paste_submit").click();
}

/** Pairs the two pages and waits until both show a connected link. */
export async function pairWeb(dual: DualContexts, signalingUrl?: string): Promise<string> {
  const code = await showCode(dual.babyPage, signalingUrl);
  await pasteCode(dual.parentPage, code);
  await expect(dual.parentPage.getByTestId("parent_ambient_bar")).toHaveAttribute(
    "data-kind",
    "connected",
    { timeout: 60_000 },
  );
  await expect(dual.babyPage.getByTestId("baby_status")).toHaveAttribute(
    "data-state",
    "connected",
    {
      timeout: 60_000,
    },
  );
  return code;
}

/** The parent's ambient bar kind: connected, reconnecting, cant_reach, lost_for, baby_stopped, ... */
export async function waitForBarKind(
  page: Page,
  kind: string | RegExp,
  timeout = 60_000,
): Promise<void> {
  await expect(page.getByTestId("parent_ambient_bar")).toHaveAttribute("data-kind", kind, {
    timeout,
  });
}

/** Fails with the violations (rule id and target) when axe finds any on the current screen. */
export async function expectNoAxeViolations(page: Page): Promise<void> {
  const { violations } = await new AxeBuilder({ page }).analyze();
  expect(
    violations.map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(" ")).join(" | ")}`),
  ).toEqual([]);
}
