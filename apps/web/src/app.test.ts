import { render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import App from "./App.svelte";
import { createFakeController } from "./lib/contract/fake-controller";
import { locale } from "./lib/i18n/store";
import { createTranslator } from "./lib/i18n/translate";

describe("app shell", () => {
  afterEach(() => {
    window.history.replaceState(null, "", "/");
    locale.set("en");
    delete document.documentElement.dataset.theme;
  });

  it("renders the main landmark, a skip link and the route heading", async () => {
    render(App, { controller: createFakeController() });
    expect(screen.getByRole("main")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Skip to content" })).toHaveAttribute("href", "#main");
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Which device is this?");
    await waitFor(() => expect(document.title).toBe("Which device is this? · GuardedSleep"));
    expect(document.documentElement.lang).toBe("en");
  });

  it("opens on the path in the address bar", async () => {
    window.history.replaceState(null, "", "/parent/pair");
    render(App, { controller: createFakeController() });
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Pair the parent device");
    await waitFor(() => expect(document.title).toContain("Pair the parent device"));
  });

  it("moves focus to the heading on navigation", async () => {
    render(App, { controller: createFakeController() });
    window.history.pushState(null, "", "/settings");
    window.dispatchEvent(new PopStateEvent("popstate"));
    const heading = await screen.findByRole("heading", { level: 1, name: "Settings" });
    await waitFor(() => expect(document.activeElement).toBe(heading));
  });

  it("follows the language setting and falls back to the browser language", async () => {
    const controller = createFakeController();
    render(App, { controller });
    controller.settings.update((s) => ({ ...s, language: "de" }));
    await waitFor(() =>
      expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
        createTranslator("de")("webShell.route.role"),
      ),
    );
    expect(document.documentElement.lang).toBe("de");
    controller.settings.update((s) => ({ ...s, language: null }));
    // jsdom reports en-US.
    await waitFor(() => expect(document.documentElement.lang).toBe("en"));
  });

  it("applies the theme setting, and dims the monitoring baby device", async () => {
    const controller = createFakeController();
    render(App, { controller });
    controller.settings.update((s) => ({ ...s, theme: "light" }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
    controller.app.update((s) => ({ ...s, role: "baby", monitoring: true }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dim"));
    controller.app.update((s) => ({ ...s, monitoring: false }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
  });
});
