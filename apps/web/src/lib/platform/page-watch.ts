import type { UnitWarning } from "@guardedsleep/core-wasm";
import { readable, writable, type Readable } from "svelte/store";
import type { AwayReport, PageGuardState } from "../contract";
import type { Cancel, Clock, Timers } from "./clock";
import {
  AWAY_REPORT_MIN_MS,
  BACKGROUND_GAP_MS,
  GAP_CHECK_MS,
  RESUMED_WARNING_MS,
  SLEEP_GAP_MS,
} from "./policy";

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
  /** The same clock the core runs on: it keeps counting while the device sleeps. */
  clock: Clock;
  timers: Timers;
}

/**
 * Why a page that is monitoring may stop doing so, reported to the user and (baby) to the parent.
 * Pure browser glue: it feeds the core nothing but "the network may have changed" after a gap, and
 * decides no alarm. The core sees a sleep as elapsed time through the clock and escalates itself.
 */
export interface PageWatch {
  readonly state: Readable<PageGuardState>;
  /** Monitoring started or stopped: takes or gives up the wake lock and the leave-page prompt. */
  setMonitoring(on: boolean): void;
  /** Tries to take the wake lock again (after a release or a refusal). */
  retryKeepAwake(): void;
  dismissAway(): void;
  /** The baby device's reliability warnings for the parent, in the core's wire names. */
  unitWarnings(): UnitWarning[];
  /** Called after a gap in the page's own timers or a return from the background. */
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
  const { doc, win, clock, timers } = deps;
  const initial: PageGuardState = { wakeLock: "off", hidden: false, away: null };
  const store = writable<PageGuardState>(initial);
  let state = initial;
  let monitoring = false;
  let sentinel: WakeLockSentinelLike | null = null;
  let requesting = false;
  let hiddenSinceWall: number | null = null;
  let resumedAtWall: number | null = null;
  let lastLook = clock.elapsedMs();
  let stopLooking: Cancel | null = null;
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

  /**
   * Looks at how long it was since the last look. A hidden page's timers run once a minute at
   * most, so only a much longer silence counts there; a visible page that was silent for 10 s
   * slept (or was suspended). Returns the silent time when it counts as a gap, else 0.
   */
  function look(wasHidden: boolean): number {
    const now = clock.elapsedMs();
    const silent = now - lastLook;
    lastLook = now;
    return silent >= (wasHidden ? BACKGROUND_GAP_MS : SLEEP_GAP_MS) ? silent : 0;
  }

  function onGap(silentMs: number, wasHidden: boolean): void {
    if (!monitoring) return;
    // The sockets did not survive the gap: let the core retry at once.
    noteResumed();
    if (wasHidden) return; // the return from the background reports it
    resumedAtWall = clock.wallMs();
    set({ away: { kind: "sleep", awayMs: silentMs, endWallMs: clock.wallMs() } });
  }

  function onTimer(): void {
    const wasHidden = hidden();
    const gap = look(wasHidden);
    if (gap > 0) onGap(gap, wasHidden);
  }

  function onVisibility(): void {
    const nowHidden = hidden();
    // The look belongs to the state the page was in until now.
    const gap = look(!nowHidden);
    const nowWall = clock.wallMs();
    if (nowHidden) {
      hiddenSinceWall = nowWall;
      set({ hidden: true });
      if (gap > 0) onGap(gap, false);
      return;
    }
    const since = hiddenSinceWall;
    hiddenSinceWall = null;
    if (monitoring && since !== null) {
      const awayMs = nowWall - since;
      const away: AwayReport | null =
        awayMs >= AWAY_REPORT_MIN_MS ? { kind: "background", awayMs, endWallMs: nowWall } : null;
      set({ hidden: false, ...(away ? { away } : {}) });
      noteResumed();
    } else {
      set({ hidden: false });
    }
    acquire();
  }

  function onBeforeUnload(event: BeforeUnloadEvent): void {
    if (!monitoring) return;
    // The browser shows its own confirmation; the text of the page cannot be customised.
    event.preventDefault();
    event.returnValue = "";
  }

  // The page can be restored from the back/forward cache or regain focus after a sleep.
  const onWake = (): void => onTimer();

  doc.addEventListener("visibilitychange", onVisibility);
  win.addEventListener("beforeunload", onBeforeUnload as EventListener);
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
        lastLook = clock.elapsedMs();
        stopLooking = timers.every(onTimer, GAP_CHECK_MS);
        acquire();
        return;
      }
      stopLooking?.();
      stopLooking = null;
      const old = sentinel;
      sentinel = null;
      if (old) void old.release().catch(() => undefined);
      resumedAtWall = null;
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
      stopLooking?.();
      stopLooking = null;
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
