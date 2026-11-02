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

/**
 * Page visibility, focus, notifications and a skewable clock for the background specs.
 * `window.__guard.setHidden(true)` behaves like switching to another tab; `skew(ms)` moves
 * `Date.now()` forward without any timer running, like a laptop that slept; `shown` lists the
 * notifications the page raised and whether they were closed again.
 */
export interface GuardProbe {
  setHidden(hidden: boolean): void;
  skew(ms: number): void;
  shown: Array<{ title: string; body: string; tag: string; closed: boolean }>;
}

export const pageGuardStub = (): void => {
  let hidden = false;
  let skew = 0;
  const realNow = Date.now.bind(Date);
  Date.now = () => realNow() + skew;
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    get: () => (hidden ? "hidden" : "visible"),
  });
  Object.defineProperty(document, "hidden", { configurable: true, get: () => hidden });
  document.hasFocus = () => !hidden;

  const shown: GuardProbe["shown"] = [];
  class FakeNotification {
    static permission = "granted";
    static requestPermission = () => Promise.resolve("granted");
    private readonly entry: GuardProbe["shown"][number];
    constructor(title: string, options?: NotificationOptions) {
      this.entry = {
        title,
        body: options?.body ?? "",
        tag: options?.tag ?? "",
        closed: false,
      };
      shown.push(this.entry);
    }
    close(): void {
      this.entry.closed = true;
    }
  }
  (window as unknown as { Notification: unknown }).Notification = FakeNotification;

  const probe: GuardProbe = {
    setHidden(value) {
      hidden = value;
      document.dispatchEvent(new Event("visibilitychange"));
    },
    skew(ms) {
      skew += ms;
    },
    shown,
  };
  (window as unknown as { __guard: GuardProbe }).__guard = probe;
};

export async function setHidden(page: Page, hidden: boolean): Promise<void> {
  await page.evaluate(
    (value) => (window as unknown as { __guard: GuardProbe }).__guard.setHidden(value),
    hidden,
  );
}

export async function skewClock(page: Page, ms: number): Promise<void> {
  await page.evaluate(
    (value) => (window as unknown as { __guard: GuardProbe }).__guard.skew(value),
    ms,
  );
}

export async function shownNotifications(page: Page): Promise<GuardProbe["shown"]> {
  return page.evaluate(() =>
    (window as unknown as { __guard: GuardProbe }).__guard.shown.map((n) => ({ ...n })),
  );
}

/** A recorded `securitypolicyviolation` event. */
export interface CspViolation {
  directive: string;
  blocked: string;
  sample: string;
}

/** Records every CSP and Trusted Types violation of the page in `window.__csp`. */
export const cspProbeStub = (): void => {
  const seen: CspViolation[] = [];
  (window as unknown as { __csp: CspViolation[] }).__csp = seen;
  document.addEventListener("securitypolicyviolation", (event) => {
    seen.push({
      directive: event.effectiveDirective,
      blocked: event.blockedURI,
      sample: event.sample,
    });
  });
};

export async function cspViolations(page: Page): Promise<CspViolation[]> {
  return page.evaluate(() => [...(window as unknown as { __csp: CspViolation[] }).__csp]);
}

export async function revokeWakeLocks(page: Page): Promise<void> {
  await page.evaluate(() =>
    (window as unknown as { __wakeLock: WakeLockProbe }).__wakeLock.revokeAll(),
  );
}
