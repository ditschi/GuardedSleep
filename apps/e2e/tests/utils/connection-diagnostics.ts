import type { Page } from "@playwright/test";

export type ConnectionDiagnosticsSnapshot = {
  pageUrl: string;
  barState: string | null;
  barLevel: string | null;
  pcState: string | null;
};

export async function readConnectionDiagnostics(
  page: Page,
): Promise<ConnectionDiagnosticsSnapshot> {
  return page.evaluate(() => {
    const bar = document.querySelector(
      '[data-testid="parent_ambient_bar"],[data-testid="baby_status"]',
    );
    const pc = window.__guardedsleep_test?.getSession?.()?.getE2ePeerConnection?.() ?? null;
    return {
      pageUrl: window.location.href,
      barState: bar?.getAttribute("data-state") ?? null,
      barLevel: bar?.getAttribute("data-level") ?? null,
      pcState: pc?.connectionState ?? null,
    };
  });
}

export async function logConnectionDiagnostics(
  label: string,
  babyPage: Page,
  parentPage: Page,
): Promise<void> {
  const [baby, parent] = await Promise.all([
    readConnectionDiagnostics(babyPage),
    readConnectionDiagnostics(parentPage),
  ]);
  console.info(`[e2e] connection diagnostics (${label})`, { baby, parent });
}
