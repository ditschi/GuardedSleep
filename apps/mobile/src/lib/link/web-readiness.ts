import { readable, writable, type Readable } from "svelte/store";

/**
 * Browser limits made visible: the page can only watch over the child while it is open, visible
 * (or at least not frozen), awake and, for a laptop, plugged in.
 */

type WakeLockSentinelLike = {
  release(): Promise<void>;
  addEventListener(t: "release", cb: () => void): void;
};

export type WakeLockHandle = {
  active: Readable<boolean>;
  supported: boolean;
  /** Request the lock; it is re-acquired every time the tab becomes visible again. */
  start(): void;
  stop(): void;
};

export function createWakeLock(): WakeLockHandle {
  const active = writable(false);
  const nav =
    typeof navigator === "undefined"
      ? undefined
      : (navigator as Navigator & {
          wakeLock?: { request(type: "screen"): Promise<WakeLockSentinelLike> };
        });
  const supported = Boolean(nav?.wakeLock);
  let sentinel: WakeLockSentinelLike | null = null;
  let wanted = false;

  const acquire = async (): Promise<void> => {
    if (!wanted || !nav?.wakeLock || sentinel) return;
    try {
      const s = await nav.wakeLock.request("screen");
      if (!wanted) {
        void s.release().catch(() => {});
        return;
      }
      sentinel = s;
      active.set(true);
      s.addEventListener("release", () => {
        if (sentinel === s) sentinel = null;
        active.set(false);
      });
    } catch {
      active.set(false);
    }
  };
  const onVisible = (): void => {
    if (document.visibilityState === "visible") void acquire();
  };

  return {
    active: { subscribe: active.subscribe },
    supported,
    start() {
      if (wanted) return;
      wanted = true;
      document.addEventListener("visibilitychange", onVisible);
      void acquire();
    },
    stop() {
      wanted = false;
      document.removeEventListener("visibilitychange", onVisible);
      const s = sentinel;
      sentinel = null;
      active.set(false);
      void s?.release().catch(() => {});
    },
  };
}

export type VisibilityState = {
  hidden: boolean;
  /** The page was hidden and is visible again: alerts may have been missed meanwhile. */
  returnedAfterHidden: boolean;
};

export type VisibilityTracker = {
  state: Readable<VisibilityState>;
  dismissReturnWarning(): void;
  destroy(): void;
};

export function trackVisibility(): VisibilityTracker {
  const state = writable<VisibilityState>({
    hidden: typeof document !== "undefined" && document.visibilityState === "hidden",
    returnedAfterHidden: false,
  });
  const onChange = (): void => {
    const hidden = document.visibilityState === "hidden";
    state.update((s) => ({
      hidden,
      returnedAfterHidden: s.returnedAfterHidden || (s.hidden && !hidden),
    }));
  };
  document.addEventListener("visibilitychange", onChange);
  return {
    state: { subscribe: state.subscribe },
    dismissReturnWarning: () => state.update((s) => ({ ...s, returnedAfterHidden: false })),
    destroy: () => document.removeEventListener("visibilitychange", onChange),
  };
}

/** true = charging, false = on battery, null = the browser does not tell (desktops, Safari, Firefox). */
export function watchCharging(): Readable<boolean | null> {
  return readable<boolean | null>(null, (set) => {
    const nav = navigator as Navigator & {
      getBattery?: () => Promise<EventTarget & { charging: boolean }>;
    };
    if (typeof nav.getBattery !== "function") return;
    let off = false;
    let cleanup = (): void => {};
    nav
      .getBattery()
      .then((b) => {
        if (off) return;
        const publish = (): void => set(b.charging);
        b.addEventListener("chargingchange", publish);
        cleanup = () => b.removeEventListener("chargingchange", publish);
        publish();
      })
      .catch(() => {});
    return () => {
      off = true;
      cleanup();
    };
  });
}
