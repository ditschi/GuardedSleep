import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import App from "../App.svelte";
import { defaultLinkUi } from "../lib/contract";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
import { locale } from "../lib/i18n/store";

function open(path: string, controller: FakeController) {
  window.history.replaceState(null, "", path);
  return render(App, { controller });
}

function controllerFor(role: "baby" | "parent", monitoring = true): FakeController {
  const c = createFakeController();
  c.app.update((s) => ({ ...s, role, paired: true, monitoring }));
  c.soundUnlocked.set(true);
  const link = { ...defaultLinkUi(), phase: "connected" as const };
  if (role === "baby") c.baby.update((b) => ({ ...b, link }));
  else c.parent.update((p) => ({ ...p, link }));
  return c;
}

const realNotification = (window as unknown as { Notification?: unknown }).Notification;

afterEach(() => {
  window.history.replaceState(null, "", "/");
  locale.set("en");
  (window as unknown as { Notification?: unknown }).Notification = realNotification;
});

describe.each(["baby", "parent"] as const)("away notice on the %s monitor", (role) => {
  it("tells how long the page was in the background and can be dismissed", async () => {
    const c = controllerFor(role);
    c.pageGuard.update((g) => ({
      ...g,
      away: { kind: "background", awayMs: 6 * 60_000, endWallMs: Date.now() },
    }));
    open(`/${role}`, c);
    const notice = await screen.findByTestId("away_notice");
    expect(notice).toHaveAttribute("data-kind", "background");
    expect(notice).toHaveTextContent("in the background for 6 min");
    expect(notice).toHaveTextContent("Alerts may have been missed");
    await fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(c.calls).toContainEqual(["dismissAwayReport"]);
    await waitFor(() => expect(screen.queryByTestId("away_notice")).toBeNull());
  });

  it("names a sleep as sleep", async () => {
    const c = controllerFor(role);
    c.pageGuard.update((g) => ({
      ...g,
      away: { kind: "sleep", awayMs: 95 * 60_000, endWallMs: Date.now() },
    }));
    open(`/${role}`, c);
    const notice = await screen.findByTestId("away_notice");
    expect(notice).toHaveAttribute("data-kind", "sleep");
    expect(notice).toHaveTextContent("device was asleep for 1 h 35 min");
  });

  it("is absent when nothing happened", async () => {
    const c = controllerFor(role);
    open(`/${role}`, c);
    await screen.findByTestId(role === "baby" ? "baby_status" : "parent_headline");
    expect(screen.queryByTestId("away_notice")).toBeNull();
  });
});

describe("notification when the page goes to the background", () => {
  let shown: Array<{ title: string; body?: string; tag?: string; closed: boolean }>;
  beforeEach(() => {
    shown = [];
    class FakeNotification {
      static permission = "granted";
      static requestPermission = () => Promise.resolve("granted");
      private readonly entry: (typeof shown)[number];
      constructor(title: string, options?: NotificationOptions) {
        this.entry = { title, body: options?.body, tag: options?.tag, closed: false };
        shown.push(this.entry);
      }
      close(): void {
        this.entry.closed = true;
      }
    }
    (window as unknown as { Notification: unknown }).Notification = FakeNotification;
    // The tab is in the background whenever the page does not have the focus.
    document.hasFocus = () => false;
  });

  it("appears while monitoring and closes when the page is back", async () => {
    const c = controllerFor("parent");
    open("/parent", c);
    await screen.findByTestId("parent_headline");
    expect(shown).toHaveLength(0);
    c.pageGuard.update((g) => ({ ...g, hidden: true }));
    await waitFor(() => expect(shown).toHaveLength(1));
    expect(shown[0]?.title).toBe("GuardedSleep may stop monitoring");
    expect(shown[0]?.tag).toBe("guardedsleep-hidden");
    c.pageGuard.update((g) => ({ ...g, hidden: false }));
    await waitFor(() => expect(shown[0]?.closed).toBe(true));
  });

  it("stays quiet when no monitoring runs", async () => {
    const c = controllerFor("baby", false);
    open("/", c);
    c.pageGuard.update((g) => ({ ...g, hidden: true }));
    await new Promise((r) => setTimeout(r, 20));
    expect(shown).toHaveLength(0);
  });
});
