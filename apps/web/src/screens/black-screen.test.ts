import { act, fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App.svelte";
import { defaultLinkUi } from "../lib/contract";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
import { locale } from "../lib/i18n/store";

function open(path: string, controller: FakeController) {
  window.history.replaceState(null, "", path);
  return render(App, { controller });
}

function controllerFor(role: "baby" | "parent"): FakeController {
  const c = createFakeController();
  c.app.update((s) => ({ ...s, role, paired: true, monitoring: true }));
  c.soundUnlocked.set(true);
  const link = { ...defaultLinkUi(), phase: "connected" as const, lastAliveElapsedMs: 1 };
  if (role === "baby") c.baby.update((b) => ({ ...b, link }));
  else c.parent.update((p) => ({ ...p, link, quietSinceWallMs: Date.now() - 5 * 60_000 }));
  return c;
}

const isOpen = (): boolean => screen.getByTestId("black_screen").hasAttribute("open");

afterEach(() => {
  vi.useRealTimers();
  window.history.replaceState(null, "", "/");
  locale.set("en");
});

describe.each([
  ["baby", "baby_black_open"],
  ["parent", "parent_black_open"],
] as const)("black screen on the %s device", (role, openId) => {
  let c: FakeController;
  beforeEach(() => {
    c = controllerFor(role);
  });

  it("is closed until asked for, then dark: no status text", async () => {
    open(`/${role}`, c);
    await screen.findByTestId(openId);
    expect(isOpen()).toBe(false);
    await fireEvent.click(screen.getByTestId(openId));
    expect(isOpen()).toBe(true);
    expect(screen.getByTestId("black_screen")).toHaveAccessibleName(/Black screen/);
    expect(screen.queryByTestId("black_screen_status")).toBeNull();
  });

  it("shows the status on the first tap and closes on the second", async () => {
    open(`/${role}`, c);
    await fireEvent.click(await screen.findByTestId(openId));
    await fireEvent.click(screen.getByTestId("black_screen_tap"));
    const status = screen.getByTestId("black_screen_status");
    expect(status).toHaveTextContent("Still monitoring");
    expect(status).toHaveTextContent("Tap again to close the black screen");
    expect(isOpen()).toBe(true);
    await fireEvent.click(screen.getByTestId("black_screen_tap"));
    expect(isOpen()).toBe(false);
  });

  it("goes dark again a few seconds after the first tap", async () => {
    open(`/${role}`, c);
    await fireEvent.click(await screen.findByTestId(openId));
    vi.useFakeTimers();
    await fireEvent.click(screen.getByTestId("black_screen_tap"));
    expect(screen.queryByTestId("black_screen_status")).not.toBeNull();
    await act(() => vi.advanceTimersByTimeAsync(7_000));
    expect(screen.queryByTestId("black_screen_status")).toBeNull();
    expect(isOpen()).toBe(true);
  });

  it("closes with Escape and gives the focus back to the button", async () => {
    open(`/${role}`, c);
    const opener = await screen.findByTestId(openId);
    opener.focus();
    await fireEvent.click(opener);
    const dialog = screen.getByTestId("black_screen") as HTMLDialogElement;
    // jsdom has no Escape handling for dialogs: the browser closes it and fires `close`.
    dialog.close();
    expect(isOpen()).toBe(false);
    expect(document.activeElement).toBe(opener);
  });
});

describe("black screen and alerts", () => {
  it("the status line is the headline on the parent device", async () => {
    const c = controllerFor("parent");
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("parent_black_open"));
    await fireEvent.click(screen.getByTestId("black_screen_tap"));
    expect(screen.getByTestId("black_screen_status")).toHaveTextContent(/All quiet since/);
  });

  it("a noise alert replaces the black screen and it stays off afterwards", async () => {
    const c = controllerFor("parent");
    open("/parent", c);
    await fireEvent.click(await screen.findByTestId("parent_black_open"));
    expect(isOpen()).toBe(true);
    c.parent.update((p) => ({ ...p, noiseAlertActive: true, noiseSinceWallMs: Date.now() }));
    expect(await screen.findByTestId("alert_title")).toBeInTheDocument();
    expect(screen.queryByTestId("black_screen")).toBeNull();
    await fireEvent.click(screen.getByTestId("alert_stop_sound"));
    await screen.findByTestId("parent_headline");
    expect(isOpen()).toBe(false);
  });

  it("the baby status line follows the connection", async () => {
    const c = controllerFor("baby");
    c.baby.update((b) => ({ ...b, link: { ...b.link, phase: "reconnecting", level: 1 } }));
    open("/baby", c);
    await fireEvent.click(await screen.findByTestId("baby_black_open"));
    await fireEvent.click(screen.getByTestId("black_screen_tap"));
    expect(screen.getByTestId("black_screen_status")).toHaveTextContent(
      "Reconnecting to parent device",
    );
  });
});
