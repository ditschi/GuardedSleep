import type { Page } from "@playwright/test";

/*
 * Browser API stubs for the page-guard specs (25-28). Each `*Stub` function is the body of a
 * `context.addInitScript`: it runs inside the page, so it must not use anything from this module.
 */

/** What the stubbed Screen Wake Lock records (see `wakeLockStub`). */
export interface WakeLockProbe {
  requests: number;
  /** Locks taken and not yet released. */
  active: number;
  /** Make the next requests fail like a browser that refuses (battery saver, policy). */
  refuse: boolean;
  /** The system takes every active lock away (fires the sentinels' `release` event). */
  revokeAll(): void;
}

/** A `navigator.wakeLock` that counts requests and can be revoked from the test. */
export const wakeLockStub = (): void => {
  const probe = {
    requests: 0,
    active: 0,
    refuse: false,
    sentinels: [] as Array<{ fire(): void }>,
    revokeAll() {
      for (const s of this.sentinels.splice(0)) s.fire();
    },
  };
  (window as unknown as { __wakeLock: typeof probe }).__wakeLock = probe;
  Object.defineProperty(navigator, "wakeLock", {
    configurable: true,
    value: {
      request: async () => {
        if (probe.refuse) throw new DOMException("refused", "NotAllowedError");
        probe.requests++;
        probe.active++;
        const listeners: Array<() => void> = [];
        let live = true;
        const sentinel = {
          addEventListener: (_type: string, l: () => void) => listeners.push(l),
          release: async () => {
            if (live) probe.active--;
            live = false;
          },
          fire() {
            if (live) probe.active--;
            live = false;
            for (const l of listeners) l();
          },
        };
        probe.sentinels.push(sentinel);
        return sentinel;
      },
    },
  });
};

/** No `navigator.wakeLock` at all (Firefox before 126, plain http). */
export const noWakeLockStub = (): void => {
  Object.defineProperty(navigator, "wakeLock", { configurable: true, value: undefined });
};

export async function wakeLockCounts(page: Page): Promise<{ requests: number; active: number }> {
  return page.evaluate(() => {
    const p = (window as unknown as { __wakeLock: WakeLockProbe }).__wakeLock;
    return { requests: p.requests, active: p.active };
  });
}

export async function revokeWakeLocks(page: Page): Promise<void> {
  await page.evaluate(() =>
    (window as unknown as { __wakeLock: WakeLockProbe }).__wakeLock.revokeAll(),
  );
}
