import { fireEvent, render, screen } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
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
  const link = { ...defaultLinkUi(), phase: "connected" as const };
  if (role === "baby") c.baby.update((b) => ({ ...b, link }));
  else c.parent.update((p) => ({ ...p, link }));
  return c;
}

afterEach(() => {
  window.history.replaceState(null, "", "/");
  locale.set("en");
});

/** The baby screen has its own notice; the parent's is the banner `browser_keep_open` of the stack. */
const warningId = (role: "baby" | "parent", state: "lost" | "unsupported"): string =>
  role === "baby" ? "wake_lock_warning" : `parent_banner_browser_keep_open_${state}`;
const retryId = (role: "baby" | "parent"): string =>
  role === "baby" ? "wake_lock_retry" : "parent_banner_browser_keep_open_lost_action";

describe.each(["baby", "parent"] as const)("keep-awake notice on the %s monitor", (role) => {
  it("is absent while the wake lock is held", async () => {
    const c = controllerFor(role);
    c.pageGuard.update((g) => ({ ...g, wakeLock: "held" }));
    open(`/${role}`, c);
    await screen.findByTestId(role === "baby" ? "baby_status" : "parent_headline");
    expect(screen.queryByTestId("wake_lock_warning")).toBeNull();
    expect(screen.queryByTestId("parent_banner_browser_keep_open_lost")).toBeNull();
  });

  it("warns when the lock is lost and takes it again on request", async () => {
    const c = controllerFor(role);
    c.pageGuard.update((g) => ({ ...g, wakeLock: "lost" }));
    open(`/${role}`, c);
    const warning = await screen.findByTestId(warningId(role, "lost"));
    expect(warning).toHaveTextContent("no longer kept on");
    await fireEvent.click(screen.getByTestId(retryId(role)));
    expect(c.calls).toContainEqual(["retryKeepAwake"]);
  });

  it("says plainly when the browser cannot keep the screen on", async () => {
    const c = controllerFor(role);
    c.pageGuard.update((g) => ({ ...g, wakeLock: "unsupported" }));
    open(`/${role}`, c);
    const warning = await screen.findByTestId(warningId(role, "unsupported"));
    expect(warning).toHaveTextContent("can't keep the screen on");
    expect(screen.queryByTestId(retryId(role))).toBeNull();
  });
});

describe("baby monitor", () => {
  it("shows that the screen is kept on", async () => {
    const c = controllerFor("baby");
    c.pageGuard.update((g) => ({ ...g, wakeLock: "held" }));
    open("/baby", c);
    expect(await screen.findByTestId("baby_wake_lock")).toHaveTextContent("Screen is kept on");
  });
});
