import type { Page } from "@playwright/test";
import type { DualContexts } from "./dual-context";

/** The baby unit's start button (setup checklist); a no-op when monitoring already runs. */
export async function startSession(page: Page): Promise<void> {
  const btn = page.getByTestId("session-start");
  const running = page.getByTestId("baby_status");
  await Promise.race([
    btn.waitFor({ state: "visible", timeout: 20_000 }),
    running.waitFor({ state: "visible", timeout: 20_000 }),
  ]);
  if (await btn.isVisible()) await btn.click();
}

/** Parent: tap the sound gate when the browser did not allow autoplay by itself. */
export async function startParent(page: Page): Promise<void> {
  const gate = page.getByTestId("parent_sound_gate");
  const barUp = page.getByTestId("parent_ambient_bar");
  await Promise.race([
    barUp.waitFor({ state: "visible", timeout: 10_000 }),
    gate.waitFor({ state: "visible", timeout: 10_000 }),
  ]).catch(() => {});
  if (await gate.isVisible().catch(() => false)) await gate.click();
}

export async function readInvite(babyPage: Page): Promise<string> {
  await babyPage.waitForFunction(
    () => (window.__guardedsleep_test?.getInviteJson?.() ?? "").length > 20,
    undefined,
    { timeout: 30_000 },
  );
  return babyPage.evaluate(() => window.__guardedsleep_test!.getInviteJson!());
}

/** Parent: paste a pairing code on /pair and land on /parent (client-side navigation). */
export async function pasteInvite(parentPage: Page, invite: string): Promise<void> {
  await parentPage.goto("/pair?role=parent");
  await parentPage.getByTestId("parent_pair_paste_field").fill(invite);
  await parentPage.getByTestId("parent_pair_paste_submit").click();
  await parentPage.waitForURL(/\/parent$/);
  await startParent(parentPage);
}

/** Baby shows a fresh v3 code, the parent pastes it. The PSK never touches storage. */
export async function pairDual(
  dual: DualContexts,
  opts?: { syntheticLevels?: boolean },
): Promise<string> {
  await dual.babyPage.goto(opts?.syntheticLevels ? "/baby?e2e_synthetic_levels=1" : "/baby");
  await startSession(dual.babyPage);
  const invite = await readInvite(dual.babyPage);
  await pasteInvite(dual.parentPage, invite);
  return invite;
}

const defaultSignalUrl = (): string => process.env.E2E_SIGNALING_URL ?? "ws://127.0.0.1:18787";

/**
 * Legacy v2 deep links. The web client ignores them: v3 pairing needs a code, see `pairDual`.
 */
export function babyDeepLink(
  room: string,
  signal = defaultSignalUrl(),
  extra?: Record<string, string>,
): string {
  const q = new URLSearchParams({ room, signal });
  for (const [k, v] of Object.entries(extra ?? {})) q.set(k, v);
  return `/baby?${q.toString()}`;
}

export function parentDeepLink(room: string, signal = defaultSignalUrl()): string {
  return `/parent?room=${encodeURIComponent(room)}&signal=${encodeURIComponent(signal)}`;
}
