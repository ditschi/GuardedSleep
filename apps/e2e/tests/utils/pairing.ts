import type { Page } from "@playwright/test";

const defaultSignalUrl = (): string => process.env.E2E_SIGNALING_URL ?? "ws://127.0.0.1:18787";

export function babyDeepLink(room: string, signal = defaultSignalUrl()): string {
  return `/baby?room=${encodeURIComponent(room)}&signal=${encodeURIComponent(signal)}`;
}

export function parentDeepLink(room: string, signal = defaultSignalUrl()): string {
  return `/parent?room=${encodeURIComponent(room)}&signal=${encodeURIComponent(signal)}`;
}

export async function startSession(page: Page): Promise<void> {
  await page.getByTestId("session-start").click();
}
