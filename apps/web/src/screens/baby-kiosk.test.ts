import { render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import App from "../App.svelte";
import { defaultLinkUi, type LinkUi } from "../lib/contract";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
import { locale } from "../lib/i18n/store";

function open(controller: FakeController) {
  window.history.replaceState(null, "", "/baby");
  return render(App, { controller });
}

const link = (over: Partial<LinkUi>): LinkUi => ({ ...defaultLinkUi(), ...over });

/** The baby device's status line and edge mark (`connection-status`, web counterpart of slice 5). */
describe("web baby: status line and shape-coded marks", () => {
  let c: FakeController;
  beforeEach(() => {
    c = createFakeController();
    c.app.update((s) => ({ ...s, role: "baby", paired: true, monitoring: true }));
    c.baby.update((b) => ({ ...b, link: link({ phase: "connected" }) }));
  });
  afterEach(() => {
    window.history.replaceState(null, "", "/");
    locale.set("en");
  });

  it("names the state and the connection, with a short bar while connected", async () => {
    c.app.update((s) => ({
      ...s,
      connections: [{ id: "1", name: "Travel cot", isDefault: true, selected: true }],
    }));
    open(c);
    expect(await screen.findByTestId("baby_kiosk_status")).toHaveTextContent(
      "Listening · Travel cot",
    );
    expect(screen.getByTestId("baby_kiosk_edge")).toHaveAttribute("data-shape", "bar");
    expect(screen.getByTestId("baby_kiosk_mark_bar")).toBeInTheDocument();
    expect(screen.queryByTestId("baby_kiosk_mark_frame")).toBeNull();
    // Decorative: the words say it.
    expect(screen.getByTestId("baby_kiosk_edge")).toHaveAttribute("aria-hidden", "true");
  });

  it("is the plain state without a connection name", async () => {
    open(c);
    expect(await screen.findByTestId("baby_kiosk_status")).toHaveTextContent(/^Listening$/);
  });

  it.each([
    [0, "none", "baby_kiosk_mark_bar"],
    [1, "dashed", "baby_kiosk_mark_dashed"],
    [2, "frame", "baby_kiosk_mark_frame"],
  ] as const)("level %s of a drop shows shape %s", async (level, shape, mark) => {
    c.baby.update((b) => ({
      ...b,
      link: link({ phase: "reconnecting", level, lastAliveElapsedMs: 1 }),
    }));
    open(c);
    await screen.findByTestId("baby_kiosk_status");
    expect(screen.getByTestId("baby_kiosk_edge")).toHaveAttribute("data-shape", shape);
    if (shape === "none") expect(screen.queryByTestId(mark)).toBeNull();
    else expect(screen.getByTestId(mark)).toBeInTheDocument();
  });

  it("adds the square and the reason when the charger was pulled", async () => {
    // The controller keeps "charger seen" (its own test); the screen only renders it.
    c.baby.update((b) => ({ ...b, chargerSeen: true }));
    open(c);
    await screen.findByTestId("baby_kiosk_status");
    c.deviceBattery.set({ pct: 79, charging: false });
    const reason = await screen.findByTestId("baby_kiosk_attention");
    expect(reason).toHaveTextContent("Needs attention: not charging · 79%");
    expect(screen.getByTestId("baby_kiosk_mark_attention")).toBeInTheDocument();
    expect(screen.getByTestId("baby_kiosk_mark_bar")).toBeInTheDocument();
  });

  it("hides the line and the mark during a talk", async () => {
    open(c);
    await screen.findByTestId("baby_kiosk_status");
    c.baby.update((b) => ({ ...b, parentTalking: true }));
    await waitFor(() => expect(screen.queryByTestId("baby_kiosk_status")).toBeNull());
    expect(screen.queryByTestId("baby_kiosk_edge")).toBeNull();
    expect(screen.getByTestId("baby_talking")).toBeInTheDocument();
  });
});
