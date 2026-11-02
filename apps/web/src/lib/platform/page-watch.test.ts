import { get } from "svelte/store";
import { describe, expect, it } from "vitest";
import {
  createPageWatch,
  type PageWatch,
  type WakeLockApi,
  type WakeLockSentinelLike,
} from "./page-watch";
import { FakeTime } from "./test-support";

class FakeSentinel implements WakeLockSentinelLike {
  released = false;
  private listeners: Array<() => void> = [];
  addEventListener(_type: "release", listener: () => void): void {
    this.listeners.push(listener);
  }
  release(): Promise<void> {
    this.released = true;
    return Promise.resolve();
  }
  /** The system takes the lock away. */
  revoke(): void {
    this.released = true;
    for (const l of this.listeners) l();
  }
}

function setup(opts: { wakeLock?: boolean; refuse?: boolean } = {}) {
  const time = new FakeTime();
  const t = { visibility: "visible" as DocumentVisibilityState };
  const target = new EventTarget();
  Object.defineProperty(target, "visibilityState", { get: () => t.visibility });
  const doc = target as unknown as Document;
  const win = new EventTarget() as unknown as Window;
  const sentinels: FakeSentinel[] = [];
  const wakeLock: WakeLockApi | undefined =
    opts.wakeLock === false
      ? undefined
      : {
          request: () => {
            if (opts.refuse) return Promise.reject(new DOMException("denied", "NotAllowedError"));
            const s = new FakeSentinel();
            sentinels.push(s);
            return Promise.resolve(s);
          },
        };
  const watch: PageWatch = createPageWatch({ doc, win, wakeLock, clock: time, timers: time });
  const setVisibility = (v: DocumentVisibilityState): void => {
    t.visibility = v;
    doc.dispatchEvent(new Event("visibilitychange"));
  };
  /** The device sleeps: the clock moves on, no timer runs. */
  const sleep = (ms: number): void => {
    time.now += ms;
  };
  const flush = (): Promise<void> => new Promise((r) => setTimeout(r, 0));
  return { time, doc, win, watch, sentinels, setVisibility, sleep, flush };
}

describe("wake lock", () => {
  it("is taken when monitoring starts and given back when it stops", async () => {
    const e = setup();
    expect(get(e.watch.state).wakeLock).toBe("off");
    e.watch.setMonitoring(true);
    await e.flush();
    expect(get(e.watch.state).wakeLock).toBe("held");
    expect(e.watch.unitWarnings()).toEqual([]);
    e.watch.setMonitoring(false);
    expect(e.sentinels[0]?.released).toBe(true);
    expect(get(e.watch.state).wakeLock).toBe("off");
  });

  it("is taken again when the page returns, without a warning for the hidden time", async () => {
    const e = setup();
    e.watch.setMonitoring(true);
    await e.flush();
    e.setVisibility("hidden");
    // The browser releases the lock of a hidden page.
    e.sentinels[0]?.revoke();
    expect(get(e.watch.state).wakeLock).toBe("held");
    e.setVisibility("visible");
    await e.flush();
    expect(e.sentinels).toHaveLength(2);
    expect(get(e.watch.state).wakeLock).toBe("held");
  });

  it("warns when the system releases it while the page is in front, and retries on request", async () => {
    const e = setup();
    e.watch.setMonitoring(true);
    await e.flush();
    e.sentinels[0]?.revoke();
    expect(get(e.watch.state).wakeLock).toBe("lost");
    expect(e.watch.unitWarnings()).toEqual(["wake_lock_off"]);
    e.watch.retryKeepAwake();
    await e.flush();
    expect(get(e.watch.state).wakeLock).toBe("held");
    expect(e.watch.unitWarnings()).toEqual([]);
  });

  it("warns when the browser refuses the lock, and when it has none", async () => {
    const refused = setup({ refuse: true });
    refused.watch.setMonitoring(true);
    await refused.flush();
    expect(get(refused.watch.state).wakeLock).toBe("lost");

    const none = setup({ wakeLock: false });
    none.watch.setMonitoring(true);
    expect(get(none.watch.state).wakeLock).toBe("unsupported");
    expect(none.watch.unitWarnings()).toEqual(["wake_lock_off"]);
  });
});

describe("visibility and away reports", () => {
  it("reports page_hidden to the parent while hidden and the time away on return", async () => {
    const e = setup();
    e.watch.setMonitoring(true);
    await e.flush();
    e.setVisibility("hidden");
    expect(e.watch.unitWarnings()).toContain("page_hidden");
    e.time.advance(6 * 60_000);
    e.setVisibility("visible");
    const s = get(e.watch.state);
    expect(s.hidden).toBe(false);
    expect(s.away).toEqual({
      kind: "background",
      awayMs: 6 * 60_000,
      endWallMs: e.time.wallMs(),
    });
    e.watch.dismissAway();
    expect(get(e.watch.state).away).toBeNull();
    expect(e.watch.unitWarnings()).not.toContain("page_hidden");
  });

  it("does not take the throttled timers of a hidden page for a sleep", async () => {
    const e = setup();
    e.watch.setMonitoring(true);
    await e.flush();
    e.setVisibility("hidden");
    // Chrome runs a hidden page's timers once a minute.
    for (let i = 0; i < 10; i++) {
      e.sleep(59_000);
      e.time.advance(1_000);
    }
    expect(e.watch.unitWarnings()).not.toContain("resumed_from_sleep");
    expect(get(e.watch.state).away).toBeNull();
  });

  it("stays quiet about a short glance away and when nothing is monitored", async () => {
    const e = setup();
    e.watch.setMonitoring(true);
    await e.flush();
    e.setVisibility("hidden");
    e.time.advance(3_000);
    e.setVisibility("visible");
    expect(get(e.watch.state).away).toBeNull();

    e.watch.setMonitoring(false);
    e.setVisibility("hidden");
    e.time.advance(10 * 60_000);
    e.setVisibility("visible");
    expect(get(e.watch.state).away).toBeNull();
  });

  it("asks for the leave-page confirmation only while monitoring", () => {
    const e = setup();
    const event = () => {
      const ev = new Event("beforeunload", { cancelable: true });
      e.win.dispatchEvent(ev);
      return ev.defaultPrevented;
    };
    expect(event()).toBe(false);
    e.watch.setMonitoring(true);
    expect(event()).toBe(true);
    e.watch.setMonitoring(false);
    expect(event()).toBe(false);
  });
});

describe("resume from sleep", () => {
  it("reports the gap, warns the parent for a while and lets the core retry", async () => {
    const e = setup();
    let resumed = 0;
    e.watch.onResumed(() => resumed++);
    e.watch.setMonitoring(true);
    await e.flush();
    e.time.advance(5_000);
    // The laptop sleeps for 40 minutes with the page still visible; the first timer after the
    // wake-up finds the clock 40 minutes further.
    e.sleep(40 * 60_000);
    e.time.advance(1_000);
    const away = get(e.watch.state).away;
    expect(away?.kind).toBe("sleep");
    expect(away?.awayMs).toBeGreaterThanOrEqual(40 * 60_000);
    expect(away?.endWallMs).toBeLessThanOrEqual(e.time.wallMs());
    expect(resumed).toBe(1);
    expect(e.watch.unitWarnings()).toEqual(["resumed_from_sleep"]);
    e.time.advance(11 * 60_000);
    expect(e.watch.unitWarnings()).toEqual([]);
  });

  it("notices the wake-up on the page's own events before the next timer", async () => {
    const e = setup();
    e.watch.setMonitoring(true);
    await e.flush();
    e.sleep(20 * 60_000);
    e.win.dispatchEvent(new Event("focus"));
    expect(get(e.watch.state).away?.kind).toBe("sleep");
  });

  it("calls a long silence of a hidden page a stay in the background", async () => {
    const e = setup();
    let resumed = 0;
    e.watch.onResumed(() => resumed++);
    e.watch.setMonitoring(true);
    await e.flush();
    e.setVisibility("hidden");
    e.sleep(30 * 60_000);
    e.setVisibility("visible");
    expect(get(e.watch.state).away?.kind).toBe("background");
    expect(resumed).toBe(1);
  });

  it("does nothing when it is not monitoring", () => {
    const e = setup();
    e.sleep(60 * 60_000);
    e.time.advance(1_000);
    e.win.dispatchEvent(new Event("focus"));
    expect(get(e.watch.state).away).toBeNull();
    expect(e.watch.unitWarnings()).toEqual([]);
  });
});
