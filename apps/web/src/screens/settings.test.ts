import { fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { get } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import App from "../App.svelte";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
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

  describe("TURN server", () => {
    beforeEach(() => {
      c.settings.update((s) => ({ ...s, advancedEnabled: true }));
    });

    async function fill(url: string, user: string, pass: string): Promise<void> {
      await fireEvent.input(await screen.findByTestId("settings_turn"), { target: { value: url } });
      await fireEvent.input(screen.getByTestId("settings_turn_user"), { target: { value: user } });
      await fireEvent.input(screen.getByTestId("settings_turn_pass"), { target: { value: pass } });
      await fireEvent.click(screen.getByTestId("settings_turn_save"));
    }

    it("masks the password and offers no Remove before anything is saved", async () => {
      open(c);
      expect(await screen.findByTestId("settings_turn_pass")).toHaveAttribute("type", "password");
      expect(screen.getByTestId("settings_turn_pass")).toHaveAttribute(
        "autocomplete",
        "new-password",
      );
      expect(screen.getByTestId("settings_turn_user")).toHaveAttribute("autocomplete", "username");
      expect(screen.queryByTestId("settings_turn_remove")).toBeNull();
      expect(screen.queryByTestId("settings_turn_pass_saved")).toBeNull();
    });

    it("rejects an address that is not turn: or turns:", async () => {
      open(c);
      await fill("stun:x", "", "");
      expect(await screen.findByTestId("settings_turn_error")).toHaveTextContent(
        "must start with turn: or turns:",
      );
      expect(get(c.settings).turnUrl).toBeNull();
    });

    it("rejects a user name without a password", async () => {
      open(c);
      await fill("turn:turn.example.org", "test-only", "");
      expect(await screen.findByTestId("settings_turn_error")).toHaveTextContent(
        "both user name and password",
      );
      expect(get(c.settings).turnUrl).toBeNull();
    });

    it("saves, clears the password field and says a password is saved", async () => {
      open(c);
      await fill(" turn:turn.example.org ", "test-only", "pw");
      expect(await screen.findByTestId("settings_turn_pass_saved")).toHaveTextContent(
        "Password saved",
      );
      expect(get(c.settings)).toMatchObject({
        turnUrl: "turn:turn.example.org",
        turnUsername: "test-only",
        turnPasswordSaved: true,
      });
      expect(screen.getByTestId("settings_turn_pass")).toHaveValue("");
      expect(screen.queryByTestId("settings_turn_error")).toBeNull();
      expect(screen.getByTestId("settings_turn_remove")).toBeInTheDocument();
    });

    it("keeps the saved password when saved again with an empty field", async () => {
      open(c);
      await fill("turn:turn.example.org", "test-only", "pw");
      await fireEvent.click(await screen.findByTestId("settings_turn_save"));
      expect(screen.queryByTestId("settings_turn_error")).toBeNull();
      expect(get(c.settings).turnPasswordSaved).toBe(true);
    });

    it("removes the entry and empties the fields", async () => {
      open(c);
      await fill("turn:turn.example.org", "test-only", "pw");
      await fireEvent.click(await screen.findByTestId("settings_turn_remove"));
      expect(c.calls).toContainEqual(["clearTurnServer"]);
      expect(get(c.settings).turnUrl).toBeNull();
      expect(screen.getByTestId("settings_turn")).toHaveValue("");
      expect(screen.getByTestId("settings_turn_user")).toHaveValue("");
      expect(screen.queryByTestId("settings_turn_remove")).toBeNull();
    });

    it("never writes the entry to browser storage", async () => {
      open(c);
      await fill("turn:turn.example.org", "turn-user-test-only", "turn-pw-test-only");
      await screen.findByTestId("settings_turn_pass_saved");
      const stored = [...Object.values(localStorage), ...Object.values(sessionStorage)].join("\n");
      expect(stored).not.toContain("turn-pw-test-only");
      expect(stored).not.toContain("turn-user-test-only");
      expect(stored).not.toContain("turn.example.org");
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
