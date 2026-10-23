import type { UnitWarning } from "@guardedsleep/core-wasm";
import { readable, writable, type Readable } from "svelte/store";
import type { AwayReport, PageGuardState } from "../contract";
import type { SleepAwareClock } from "./clock";
import { AWAY_REPORT_MIN_MS, RESUMED_WARNING_MS } from "./policy";

/** The slice of the Screen Wake Lock API the page uses (a fake in tests). */
export interface WakeLockSentinelLike {
  addEventListener(type: "release", listener: () => void): void;
  release(): Promise<void>;
}

export interface WakeLockApi {
  request(type: "screen"): Promise<WakeLockSentinelLike>;
}

export interface PageWatchDeps {
  doc: Pick<Document, "visibilityState" | "addEventListener" | "removeEventListener">;
  win: Pick<Window, "addEventListener" | "removeEventListener">;
  /** `navigator.wakeLock`; absent in Firefox before 126, Safari before 16.4 and on plain http. */
  wakeLock: WakeLockApi | undefined;
  clock: SleepAwareClock;
}

/** Why a page that is monitoring may stop doing so. Pure browser glue: no alarm decision here. */
export interface PageWatch {
  readonly state: Readable<PageGuardState>;
  /** Monitoring started or stopped: takes or gives up the wake lock and the leave-page prompt. */
  setMonitoring(on: boolean): void;
  /** Tries to take the wake lock again (after a release or a refusal). */
  retryKeepAwake(): void;
  dismissAway(): void;
  /** The baby device's reliability warnings for the parent, in the core's wire names. */
  unitWarnings(): UnitWarning[];
  /** Called after the device slept or the page came back to the front while monitoring. */
  onResumed(fn: () => void): () => void;
  stop(): void;
}

/** A watch for pages without a browser (tests, the fake controller): nothing is ever wrong. */
export function idleWatch(): PageWatch {
  const state = readable<PageGuardState>({ wakeLock: "off", hidden: false, away: null });
  return {
    state,
    setMonitoring: () => undefined,
    retryKeepAwake: () => undefined,
    dismissAway: () => undefined,
    unitWarnings: () => [],
    onResumed: () => () => undefined,
    stop: () => undefined,
  };
}

export function createPageWatch(deps: PageWatchDeps): PageWatch {
  const { doc, win, clock } = deps;
  const store = writable<PageGuardState>({ wakeLock: "off", hidden: false, away: null });
  let state: PageGuardState = { wakeLock: "off", hidden: false, away: null };
  let monitoring = false;
  let sentinel: WakeLockSentinelLike | null = null;
  let requesting = false;
  let hiddenSinceWall: number | null = null;
  let sleptMs = 0;
  let resumedAtWall: number | null = null;
  const resumed = new Set<() => void>();

  const set = (patch: Partial<PageGuardState>): void => {
    state = { ...state, ...patch };
    store.set(state);
  };
  const hidden = (): boolean => doc.visibilityState !== "visible";

  function acquire(): void {
    if (!monitoring) return;
    if (!deps.wakeLock) {
      set({ wakeLock: "unsupported" });
      return;
    }
    // Browsers refuse the lock to a hidden page; the visible return tries again.
    if (hidden() || sentinel !== null || requesting) return;
    requesting = true;
    deps.wakeLock.request("screen").then(
      (taken) => {
        requesting = false;
        if (!monitoring) {
          void taken.release().catch(() => undefined);
          return;
        }
        sentinel = taken;
        taken.addEventListener("release", () => {
          if (sentinel !== taken) return;
          sentinel = null;
          // A hidden page loses the lock by design (the hidden warning covers it); a release
          // while the page is in front means the system took the lock away.
          if (monitoring && !hidden()) set({ wakeLock: "lost" });
        });
        set({ wakeLock: "held" });
      },
      () => {
        requesting = false;
        if (monitoring) set({ wakeLock: "lost" });
      },
    );
  }

  function noteResumed(): void {
    for (const fn of [...resumed]) fn();
  }

  function onVisibility(): void {
    clock.check();
    const nowWall = clock.wallMs();
    if (hidden()) {
      hiddenSinceWall = nowWall;
      set({ hidden: true });
      return;
    }
    const since = hiddenSinceWall;
    hiddenSinceWall = null;
    const slept = sleptMs;
    sleptMs = 0;
    if (monitoring && since !== null) {
      const awayMs = nowWall - since;
      // The sleep gap is part of the wall-clock span; whichever explains the absence is named.
      const away: AwayReport | null =
        awayMs >= AWAY_REPORT_MIN_MS || slept > 0
          ? {
              kind: slept > 0 ? "sleep" : "background",
              awayMs: Math.max(awayMs, slept),
              endWallMs: nowWall,
            }
          : null;
      set({ hidden: false, ...(away ? { away } : {}) });
      noteResumed();
    } else {
      set({ hidden: false });
    }
    acquire();
  }

  const unsubscribeGap = clock.onGap((gap) => {
    if (!monitoring) return;
    resumedAtWall = gap.wallMs;
    if (hidden()) {
      sleptMs += gap.gapMs;
      // The sockets did not survive the sleep: let the core retry at once.
      noteResumed();
      return;
    }
    // A sleep while the page stayed "visible" (lid closed, screen off, no visibility event).
    set({ away: { kind: "sleep", awayMs: gap.gapMs, endWallMs: gap.wallMs } });
    noteResumed();
  });

  function onBeforeUnload(event: BeforeUnloadEvent): void {
    if (!monitoring) return;
    // The browser shows its own confirmation; the text of the page cannot be customised.
    event.preventDefault();
    event.returnValue = "";
  }

  doc.addEventListener("visibilitychange", onVisibility);
  win.addEventListener("beforeunload", onBeforeUnload as EventListener);
  // The page can be restored from the back/forward cache or regain focus after a sleep.
  const onWake = (): void => clock.check();
  win.addEventListener("pageshow", onWake);
  win.addEventListener("focus", onWake);
  hiddenSinceWall = hidden() ? clock.wallMs() : null;
  set({ hidden: hidden() });

  return {
    state: { subscribe: store.subscribe },
    setMonitoring(on) {
      if (on === monitoring) return;
      monitoring = on;
      if (on) {
        acquire();
        return;
      }
      const old = sentinel;
      sentinel = null;
      if (old) void old.release().catch(() => undefined);
      resumedAtWall = null;
      sleptMs = 0;
      set({ wakeLock: "off", away: null });
    },
    retryKeepAwake() {
      if (state.wakeLock === "lost") {
        set({ wakeLock: "off" });
        acquire();
      }
    },
    dismissAway() {
      set({ away: null });
    },
    unitWarnings() {
      if (!monitoring) return [];
      const warnings: UnitWarning[] = [];
      if (state.hidden) warnings.push("page_hidden");
      if (state.wakeLock === "lost" || state.wakeLock === "unsupported") {
        warnings.push("wake_lock_off");
      }
      if (resumedAtWall !== null && clock.wallMs() - resumedAtWall < RESUMED_WARNING_MS) {
        warnings.push("resumed_from_sleep");
      }
      return warnings;
    },
    onResumed(fn) {
      resumed.add(fn);
      return () => resumed.delete(fn);
    },
    stop() {
      unsubscribeGap();
      doc.removeEventListener("visibilitychange", onVisibility);
      win.removeEventListener("beforeunload", onBeforeUnload as EventListener);
      win.removeEventListener("pageshow", onWake);
      win.removeEventListener("focus", onWake);
      const old = sentinel;
      sentinel = null;
      if (old) void old.release().catch(() => undefined);
    },
  };
}
