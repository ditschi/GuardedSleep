import { afterEach, describe, expect, it, vi } from "vitest";
import {
  askForNotifications,
  flashTitle,
  notifyAway,
  pageIsAway,
  type NotificationApi,
} from "./attention";

function fakeApi(permission: NotificationPermission) {
  const shown: Array<{ title: string; options?: NotificationOptions; closed: boolean }> = [];
  const request = vi.fn().mockResolvedValue("granted");
  class Fake {
    static permission = permission;
    static requestPermission = request;
    private readonly record: { closed: boolean };
    constructor(title: string, options?: NotificationOptions) {
      this.record = { title, options, closed: false } as (typeof shown)[number];
      shown.push(this.record as (typeof shown)[number]);
    }
    close(): void {
      this.record.closed = true;
    }
  }
  return { api: Fake as unknown as NotificationApi, shown, request };
}

function fakeDoc(over: { visibilityState?: string; focused?: boolean } = {}): Document {
  return {
    title: "Parent device · GuardedSleep",
    visibilityState: over.visibilityState ?? "visible",
    hasFocus: () => over.focused ?? true,
  } as unknown as Document;
}

describe("flashTitle", () => {
  afterEach(() => vi.useRealTimers());

  it("alternates the alert text with the real title and restores it", () => {
    vi.useFakeTimers();
    const doc = fakeDoc();
    const stop = flashTitle(doc, "Sound detected");
    expect(doc.title).toBe("Sound detected");
    vi.advanceTimersByTime(1000);
    expect(doc.title).toBe("Parent device · GuardedSleep");
    vi.advanceTimersByTime(1000);
    expect(doc.title).toBe("Sound detected");
    stop();
    expect(doc.title).toBe("Parent device · GuardedSleep");
    vi.advanceTimersByTime(5000);
    expect(doc.title).toBe("Parent device · GuardedSleep");
  });
});

describe("notifyAway", () => {
  const message = { title: "Sound detected", body: "The baby device hears a sound.", tag: "alert" };

  it("notifies when allowed and the page is not in front", () => {
    const { api, shown } = fakeApi("granted");
    const close = notifyAway(api, fakeDoc({ visibilityState: "hidden" }), message);
    expect(shown).toHaveLength(1);
    expect(shown[0]?.title).toBe("Sound detected");
    expect(shown[0]?.options).toMatchObject({ tag: "alert", requireInteraction: true });
    close();
    expect(shown[0]?.closed).toBe(true);
  });

  it("also notifies when the page is visible but another window has the focus", () => {
    const { api, shown } = fakeApi("granted");
    notifyAway(api, fakeDoc({ focused: false }), message);
    expect(shown).toHaveLength(1);
  });

  it("stays quiet while the page is in front (the in-page alert is enough)", () => {
    const { api, shown } = fakeApi("granted");
    notifyAway(api, fakeDoc(), message);
    expect(shown).toHaveLength(0);
  });

  it("does nothing without permission or without the API", () => {
    const denied = fakeApi("denied");
    notifyAway(denied.api, fakeDoc({ visibilityState: "hidden" }), message);
    expect(denied.shown).toHaveLength(0);
    expect(() =>
      notifyAway(undefined, fakeDoc({ visibilityState: "hidden" }), message),
    ).not.toThrow();
  });

  it("survives a browser that only allows notifications through a service worker", () => {
    const { api } = fakeApi("granted");
    const throwing = class {
      static permission = "granted";
      constructor() {
        throw new TypeError("Illegal constructor");
      }
    } as unknown as NotificationApi;
    expect(() =>
      notifyAway(throwing, fakeDoc({ visibilityState: "hidden" }), message),
    ).not.toThrow();
    expect(api).toBeTruthy();
  });
});

describe("pageIsAway", () => {
  it("is true for a hidden or unfocused page", () => {
    expect(pageIsAway(fakeDoc())).toBe(false);
    expect(pageIsAway(fakeDoc({ visibilityState: "hidden" }))).toBe(true);
    expect(pageIsAway(fakeDoc({ focused: false }))).toBe(true);
  });
});

describe("askForNotifications", () => {
  it("asks only while the user has not answered", async () => {
    const fresh = fakeApi("default");
    await askForNotifications(fresh.api);
    expect(fresh.request).toHaveBeenCalledOnce();
    const answered = fakeApi("denied");
    await askForNotifications(answered.api);
    expect(answered.request).not.toHaveBeenCalled();
    await expect(askForNotifications(undefined)).resolves.toBeUndefined();
  });
});
