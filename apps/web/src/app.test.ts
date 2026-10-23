import { render, screen, waitFor } from "@testing-library/svelte";
import { afterEach, describe, expect, it } from "vitest";
import App from "./App.svelte";
import { locale } from "./lib/i18n/store";

describe("app shell", () => {
  afterEach(() => {
    window.history.replaceState(null, "", "/");
    locale.set("en");
  });

  it("renders the main landmark, a skip link and the route heading", async () => {
    render(App);
    expect(screen.getByRole("main")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Skip to content" })).toHaveAttribute("href", "#main");
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Which device is this?");
    await waitFor(() => expect(document.title).toBe("Which device is this? · GuardedSleep"));
    expect(document.documentElement.lang).toBe("en");
  });

  it("opens on the path in the address bar", async () => {
    window.history.replaceState(null, "", "/parent/pair");
    render(App);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Pair the parent device");
    await waitFor(() => expect(document.title).toContain("Pair the parent device"));
  });

  it("moves focus to the heading on navigation and follows the locale", async () => {
    render(App);
    window.history.pushState(null, "", "/settings");
    window.dispatchEvent(new PopStateEvent("popstate"));
    const heading = await screen.findByRole("heading", { level: 1, name: "Settings" });
    await waitFor(() => expect(document.activeElement).toBe(heading));

    locale.set("de");
    await waitFor(() => expect(heading).toHaveTextContent("Einstellungen"));
    expect(document.documentElement.lang).toBe("de");
  });
});
