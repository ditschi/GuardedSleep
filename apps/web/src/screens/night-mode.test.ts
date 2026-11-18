import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { get } from "svelte/store";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import App from "../App.svelte";
import { defaultLinkUi } from "../lib/contract";
import { createFakeController, type FakeController } from "../lib/contract/fake-controller";
import { locale } from "../lib/i18n/store";
import { NIGHT_STORAGE_KEY, readNightSettings } from "../lib/night-mode";

function open(path: string, controller: FakeController) {
  window.history.replaceState(null, "", path);
  return render(App, { controller });
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  window.history.replaceState(null, "", "/");
  locale.set("en");
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.scheme;
});

describe("Settings: night mode", () => {
  it("offers Off, On and Automatic, Off by default, with the note about alerts", async () => {
    const c = createFakeController();
    open("/settings", c);
    expect(await screen.findByTestId("settings_night_off")).toBeChecked();
    expect(screen.getByTestId("settings_night_on")).not.toBeChecked();
    expect(screen.getByTestId("settings_night_auto")).not.toBeChecked();
    expect(screen.queryByTestId("settings_night_window")).toBeNull();
    expect(screen.getByTestId("settings_night_note")).toHaveTextContent(
      "Alerts and connection problems always use their normal colours.",
    );
    expect(screen.getByTestId("settings_night_status")).toHaveTextContent("Now off");
  });

  it("On applies Ember at once and the choice is remembered for the next first paint", async () => {
    const c = createFakeController();
    open("/settings", c);
    await fireEvent.click(await screen.findByTestId("settings_night_on"));
    expect(get(c.settings).nightMode).toBe("on");
    await waitFor(() => expect(document.documentElement.dataset.scheme).toBe("ember"));
    expect(readNightSettings(localStorage)?.nightMode).toBe("on");
    await fireEvent.click(screen.getByTestId("settings_night_off"));
    await waitFor(() => expect(document.documentElement.dataset.scheme).toBe("dusk"));
  });

  it("Automatic shows the window and the next change; the fields set the window", async () => {
    const c = createFakeController();
    open("/settings", c);
    await fireEvent.click(await screen.findByTestId("settings_night_auto"));
    const from = screen.getByTestId("settings_night_from") as HTMLInputElement;
    expect(from.value).toBe("21:00");
    expect((screen.getByTestId("settings_night_to") as HTMLInputElement).value).toBe("07:00");
    expect(from).toHaveAccessibleName("Night mode starts at 21:00");
    expect(screen.getByTestId("settings_night_status")).toHaveTextContent(/^Now (on|off) · turns/);
    await fireEvent.change(from, { target: { value: "22:30" } });
    expect(get(c.settings)).toMatchObject({ nightFromMinute: 22 * 60 + 30, nightToMinute: 420 });
    expect(readNightSettings(localStorage)?.nightFromMinute).toBe(22 * 60 + 30);
  });

  it("stores nothing until the user changes night mode", async () => {
    const c = createFakeController();
    open("/settings", c);
    await screen.findByTestId("settings_night_off");
    expect(localStorage.getItem(NIGHT_STORAGE_KEY)).toBeNull();
  });
});

describe("status bar in night mode", () => {
  function monitoring(level: number): FakeController {
    const c = createFakeController();
    c.app.update((s) => ({ ...s, role: "parent", paired: true, monitoring: true }));
    c.settings.update((s) => ({ ...s, nightMode: "on" }));
    c.soundUnlocked.set(true);
    const link = {
      ...defaultLinkUi(),
      phase: "reconnecting" as const,
      level,
      lastAliveElapsedMs: Date.now() - 25_000,
      downSinceElapsedMs: Date.now() - 25_000,
    };
    c.parent.update((p) => ({ ...p, link }));
    return c;
  }

  it("is the orange stage at level 1 while the page is Ember", async () => {
    open("/parent", monitoring(1));
    const bar = await screen.findByTestId("status_bar");
    expect(bar).toHaveAttribute("data-tone", "orange");
    await waitFor(() => expect(document.documentElement.dataset.scheme).toBe("ember"));
  });

  // jsdom does not apply CSS, so the stylesheet is read as text: every tone from the orange stage
  // on (and the grey one) takes only --safety-* colours, which no scheme overrides.
  const css = readFileSync(resolve(process.cwd(), "src/lib/ui/components.css"), "utf8");
  function toneColours(tone: string): string[] {
    const rules = [
      ...css.matchAll(
        new RegExp(`\\.gs-status-bar\\[data-tone="${tone}"\\]\\s*\\{([^}]*)\\}`, "g"),
      ),
    ];
    expect(rules.length, tone).toBeGreaterThan(0);
    return rules
      .flatMap((m) => [...(m[1] ?? "").matchAll(/(?:background|color|box-shadow):\s*([^;]+);/g)])
      .map((m) => m[1] ?? "");
  }

  it.each(["info", "orange", "bad", "neutral"])(
    "the %s tone uses fixed safety colours only",
    (tone) => {
      const values = toneColours(tone);
      expect(values.length).toBeGreaterThan(0);
      for (const v of values) {
        expect(v, `${tone}: ${v}`).toContain("--safety-");
        expect(v, `${tone}: ${v}`).not.toContain("--gs-");
      }
    },
  );
});
