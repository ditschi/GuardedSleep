import { fireEvent, render, screen } from "@testing-library/svelte";
import { createRawSnippet, type Snippet } from "svelte";
import { describe, expect, it, vi } from "vitest";
import Banner from "./Banner.svelte";
import Button from "./Button.svelte";
import Card from "./Card.svelte";
import Chip from "./Chip.svelte";
import Dialog from "./Dialog.svelte";
import List from "./List.svelte";
import ListRow from "./ListRow.svelte";
import Meter from "./Meter.svelte";
import StatusBar from "./StatusBar.svelte";
import Switch from "./Switch.svelte";
import { meterBars } from "./meter";

/** A snippet that renders the given text, for components that take `children`. */
function text(label: string): Snippet {
  return createRawSnippet(() => ({ render: () => `<span>${label}</span>` }));
}

describe("base components render their label", () => {
  it("Button", async () => {
    const onclick = vi.fn();
    render(Button, { children: text("Listen"), onclick, testid: "parent_listen" });
    const button = screen.getByRole("button", { name: "Listen" });
    expect(button).toHaveAttribute("data-testid", "parent_listen");
    await fireEvent.click(button);
    expect(onclick).toHaveBeenCalledOnce();
  });

  it("Button disabled", () => {
    render(Button, { children: text("Stop"), disabled: true });
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
  });

  it("Card", () => {
    render(Card, { children: text("Pairing code") });
    expect(screen.getByText("Pairing code")).toBeInTheDocument();
  });

  it("StatusBar carries tone, icon and text", () => {
    render(StatusBar, { tone: "bad", label: "Connection lost", assertive: true, testid: "bar" });
    const bar = screen.getByTestId("bar");
    expect(bar).toHaveTextContent("Connection lost");
    expect(bar).toHaveAttribute("data-tone", "bad");
    expect(bar).toHaveAttribute("role", "alert");
    expect(bar.querySelector("svg")).toHaveAttribute("aria-hidden", "true");
  });

  it("StatusBar is polite unless assertive", () => {
    render(StatusBar, { tone: "ok", label: "Connected" });
    expect(screen.getByRole("status")).toHaveTextContent("Connected");
  });

  it("Chip", () => {
    render(Chip, { tone: "warn", label: "Battery 12 %", icon: "battery" });
    expect(screen.getByText("Battery 12 %")).toBeInTheDocument();
  });

  it("Banner with dismiss", async () => {
    const onDismiss = vi.fn();
    render(Banner, {
      tone: "warn",
      message: "Keep this page open",
      dismissLabel: "Dismiss",
      onDismiss,
    });
    expect(screen.getByRole("status")).toHaveTextContent("Keep this page open");
    await fireEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it("Banner without dismiss has no button", () => {
    render(Banner, { tone: "bad", message: "Alerts may be missed" });
    expect(screen.getByRole("alert")).toHaveTextContent("Alerts may be missed");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("Meter exposes words, not only numbers", () => {
    render(Meter, { level: 0.5, threshold: 0.25, label: "Quiet" });
    const meter = screen.getByRole("meter", { name: "Quiet" });
    expect(meter).toHaveAttribute("aria-valuetext", "Quiet");
    expect(meter.querySelectorAll("[data-on]")).toHaveLength(4);
    expect(meter.querySelector(".gs-meter__threshold")).not.toBeNull();
  });

  it("Switch toggles", async () => {
    const onchange = vi.fn();
    render(Switch, { checked: false, label: "Silence this device", onchange });
    const toggle = screen.getByRole("switch", { name: "Silence this device" });
    expect(toggle).not.toBeChecked();
    await fireEvent.click(toggle);
    expect(toggle).toBeChecked();
    expect(onchange).toHaveBeenCalledWith(true);
  });

  it("Dialog shows title, body and actions, and closes", async () => {
    const onclose = vi.fn();
    render(Dialog, {
      open: true,
      title: "Stop monitoring?",
      onclose,
      children: text("You won't get alerts."),
      actions: text("Keep monitoring"),
    });
    const dialog = screen.getByRole("dialog", { name: "Stop monitoring?", hidden: true });
    expect(dialog).toHaveAttribute("open");
    expect(screen.getByText("You won't get alerts.")).toBeInTheDocument();
    expect(screen.getByText("Keep monitoring")).toBeInTheDocument();
    (dialog as HTMLDialogElement).close();
    expect(onclose).toHaveBeenCalledOnce();
  });

  it("List and ListRow", async () => {
    const onclick = vi.fn();
    render(List, {
      children: createRawSnippet(() => ({
        render: () => `<li><button type="button">Signaling servers</button></li>`,
      })),
    });
    expect(screen.getByRole("list")).toBeInTheDocument();

    render(ListRow, { title: "Theme", subtitle: "Dark", onclick, testid: "settings_theme" });
    await fireEvent.click(screen.getByRole("button", { name: /Theme/ }));
    expect(onclick).toHaveBeenCalledOnce();
    expect(screen.getByText("Dark")).toBeInTheDocument();
  });

  it("ListRow without action is plain text", () => {
    render(ListRow, { title: "Version", subtitle: "0.0.1" });
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText("Version")).toBeInTheDocument();
  });
});

describe("meterBars", () => {
  it("lights bars in proportion and clamps", () => {
    expect(meterBars(0).filter((b) => b.on)).toHaveLength(0);
    expect(meterBars(1).filter((b) => b.on)).toHaveLength(8);
    expect(meterBars(5).filter((b) => b.on)).toHaveLength(8);
    expect(meterBars(-1).filter((b) => b.on)).toHaveLength(0);
    expect(meterBars(Number.NaN).filter((b) => b.on)).toHaveLength(0);
  });

  it("colours the last bars as the loud zone", () => {
    const zones = meterBars(1).map((b) => b.zone);
    expect(zones).toEqual(["low", "low", "low", "low", "low", "mid", "mid", "high"]);
  });
});
