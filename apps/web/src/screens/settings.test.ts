import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { readdirSync } from "node:fs";
import { resolve } from "node:path";
import { get } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import App from "../App.svelte";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
import { ENDONYMS, LOCALES } from "../lib/i18n/bundles";
import { locale } from "../lib/i18n/store";
import { THEME_STORAGE_KEY } from "../lib/theme";
import { sensitivityOf } from "../lib/ui/sensitivity";

function open(controller: FakeController) {
  window.history.replaceState(null, "", "/settings");
  return render(App, { controller });
}

describe("S12 settings", () => {
  let c: FakeController;
  beforeEach(() => {
    c = createFakeController();
    localStorage.clear();
  });
  afterEach(() => {
    window.history.replaceState(null, "", "/");
    locale.set("en");
    delete document.documentElement.dataset.theme;
  });

  it("has a heading, a back button and the theme choices", async () => {
    open(c);
    expect(await screen.findByTestId("settings_title")).toHaveTextContent("Settings");
    expect(screen.getByTestId("settings_theme_dark")).toBeChecked();
    expect(screen.getByLabelText("Light")).toBeInTheDocument();
    expect(screen.getByLabelText("Match system")).toBeInTheDocument();
  });

  it("switches the theme, applies it and remembers it for the next first paint", async () => {
    open(c);
    await fireEvent.click(await screen.findByTestId("settings_theme_light"));
    expect(get(c.settings).theme).toBe("light");
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
    // The theme is no secret; this key is the only one the boot script reads.
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
  });

  it("lists the system default first, then every shipped locale by endonym", async () => {
    open(c);
    const select = await screen.findByTestId("settings_language");
    const options = within(select).getAllByRole("option");
    expect(options.map((o) => (o as HTMLOptionElement).value)).toEqual(["", ...LOCALES]);
    expect(options[0]).toHaveTextContent("Match system");
    expect(options.slice(1).map((o) => o.textContent)).toEqual(LOCALES.map((l) => ENDONYMS[l]));
    expect(within(select).getByRole("option", { name: "Deutsch" })).toBeInTheDocument();
    expect(within(select).getByRole("option", { name: "日本語" })).toBeInTheDocument();
    // Each name carries its own language and direction, so Arabic renders right to left.
    const arabic = within(select).getByRole("option", { name: "العربية" });
    expect(arabic).toHaveAttribute("lang", "ar");
    expect(arabic).toHaveAttribute("dir", "auto");
  });

  it("lists exactly the locales under locales/", () => {
    const shipped = readdirSync(resolve(process.cwd(), "../../locales"))
      .filter((n) => n.endsWith(".json") && n !== "critical.json" && n !== "endonyms.json")
      .map((n) => n.replace(/\.json$/, ""))
      .sort();
    expect([...LOCALES].sort()).toEqual(shipped);
  });

  it("switches the page to right-to-left for Arabic and back", async () => {
    open(c);
    await fireEvent.change(await screen.findByTestId("settings_language"), {
      target: { value: "ar" },
    });
    await waitFor(() => expect(document.documentElement.dir).toBe("rtl"));
    await fireEvent.change(screen.getByTestId("settings_language"), { target: { value: "en" } });
    await waitFor(() => expect(document.documentElement.dir).toBe("ltr"));
  });

  it("changes the language", async () => {
    open(c);
    await fireEvent.change(await screen.findByTestId("settings_language"), {
      target: { value: "de" },
    });
    expect(get(c.settings).language).toBe("de");
    await waitFor(() => expect(document.documentElement.lang).toBe("de"));
    await fireEvent.change(screen.getByTestId("settings_language"), { target: { value: "" } });
    expect(get(c.settings).language).toBeNull();
  });

  it("sets the sensitivity on the baby device only", async () => {
    c.app.update((s) => ({ ...s, role: "baby" }));
    open(c);
    const slider = await screen.findByTestId("settings_sensitivity");
    expect(slider).toHaveValue(String(sensitivityOf(get(c.settings).threshold)));
    await fireEvent.change(slider, { target: { value: "100" } });
    expect(get(c.settings).threshold).toBeLessThan(0.05);
  });

  it("points the parent device to the baby device for sensitivity", async () => {
    c.app.update((s) => ({ ...s, role: "parent" }));
    open(c);
    expect(await screen.findByTestId("settings_sensitivity_note")).toHaveTextContent(
      "set on the baby device",
    );
    expect(screen.queryByTestId("settings_sensitivity")).toBeNull();
  });

  it("hides the signaling addresses behind a warning", async () => {
    open(c);
    await screen.findByTestId("settings_title");
    expect(screen.queryByTestId("settings_url_input")).toBeNull();
    await fireEvent.click(screen.getByTestId("settings_advanced_toggle"));
    const dialog = screen.getByTestId("dialog_advanced");
    expect(dialog).toHaveTextContent("Wrong values here can stop the devices from connecting");
    await fireEvent.click(within(dialog).getByTestId("dialog_advanced_confirm"));
    expect(get(c.settings).advancedEnabled).toBe(true);
    expect(await screen.findByTestId("settings_url_input")).toBeInTheDocument();
  });

  it("puts the switch back when the warning is declined", async () => {
    open(c);
    await fireEvent.click(await screen.findByTestId("settings_advanced_toggle"));
    await fireEvent.click(screen.getByTestId("dialog_advanced_cancel"));
    expect(get(c.settings).advancedEnabled).toBe(false);
    await waitFor(() => expect(screen.getByTestId("settings_advanced_toggle")).not.toBeChecked());
  });

  describe("signaling addresses", () => {
    beforeEach(() => {
      c.settings.update((s) => ({ ...s, advancedEnabled: true }));
    });

    it("adds a valid address and lists it", async () => {
      open(c);
      const input = await screen.findByTestId("settings_url_input");
      await fireEvent.input(input, { target: { value: " ws://192.168.1.5:8787 " } });
      await fireEvent.click(screen.getByTestId("settings_url_add"));
      expect(get(c.settings).signalingUrls).toEqual(["ws://192.168.1.5:8787"]);
      expect(await screen.findByTestId("settings_url_list")).toHaveTextContent(
        "ws://192.168.1.5:8787",
      );
      expect(input).toHaveValue("");
    });

    it("rejects an address that is not ws:// or wss://", async () => {
      open(c);
      const input = await screen.findByTestId("settings_url_input");
      await fireEvent.input(input, { target: { value: "http://relay.example.test" } });
      await fireEvent.click(screen.getByTestId("settings_url_add"));
      expect(await screen.findByTestId("settings_url_error")).toHaveTextContent(
        "must start with wss:// or ws://",
      );
      expect(get(c.settings).signalingUrls).toEqual([]);
      await fireEvent.input(input, { target: { value: "wss://relay.example.test" } });
      await waitFor(() => expect(screen.queryByTestId("settings_url_error")).toBeNull());
    });

    it("does not list an address twice, and removes one", async () => {
      c.settings.update((s) => ({ ...s, signalingUrls: ["wss://a.test", "wss://b.test"] }));
      open(c);
      const input = await screen.findByTestId("settings_url_input");
      await fireEvent.input(input, { target: { value: "wss://a.test" } });
      await fireEvent.click(screen.getByTestId("settings_url_add"));
      expect(get(c.settings).signalingUrls).toEqual(["wss://a.test", "wss://b.test"]);
      await fireEvent.click(screen.getByTestId("settings_url_remove_0"));
      expect(get(c.settings).signalingUrls).toEqual(["wss://b.test"]);
    });
  });

  it("forgets the pairing after a confirmation and returns to the start screen", async () => {
    c.app.update((s) => ({ ...s, role: "parent", paired: true }));
    open(c);
    await fireEvent.click(await screen.findByTestId("settings_forget"));
    const dialog = screen.getByTestId("dialog_forget");
    expect(dialog).toHaveTextContent("Forget the baby device?");
    expect(dialog).toHaveTextContent("you'll need to scan a new code");
    await fireEvent.click(within(dialog).getByTestId("dialog_forget_confirm"));
    expect(c.calls).toContainEqual(["forgetPairing"]);
    await waitFor(() => expect(window.location.pathname).toBe("/"));
  });

  it("offers no forget action without a pairing", async () => {
    open(c);
    await screen.findByTestId("settings_title");
    expect(screen.queryByTestId("settings_forget")).toBeNull();
  });

  it("goes back to the monitoring screen while monitoring, otherwise to the start", async () => {
    c.app.update((s) => ({ ...s, role: "parent", monitoring: true }));
    open(c);
    await fireEvent.click(await screen.findByTestId("settings_back"));
    await waitFor(() => expect(window.location.pathname).toBe("/parent"));
  });

  it("goes back to the start screen without a session", async () => {
    open(c);
    await fireEvent.click(await screen.findByTestId("settings_back"));
    await waitFor(() => expect(window.location.pathname).toBe("/"));
  });
});
