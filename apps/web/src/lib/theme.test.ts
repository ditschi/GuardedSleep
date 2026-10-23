import { afterEach, describe, expect, it, vi } from "vitest";
import {
  applyTheme,
  isThemeSetting,
  resolveTheme,
  THEME_COLOR,
  storeThemeSetting,
  THEME_STORAGE_KEY,
  type ThemeSetting,
} from "./theme";

import bootScript from "../../public/theme-boot.js?raw";

describe("resolveTheme", () => {
  it("honours an explicit setting", () => {
    expect(resolveTheme("dark", false)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
  });

  it("follows the system for system", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
  });

  it("dims on the monitoring baby device regardless of the setting", () => {
    for (const s of ["dark", "light", "system"] as const) {
      expect(resolveTheme(s, false, true)).toBe("dim");
    }
  });

  it("validates stored values", () => {
    expect(isThemeSetting("system")).toBe(true);
    expect(isThemeSetting("dim")).toBe(false);
    expect(isThemeSetting(null)).toBe(false);
  });
});

describe("applyTheme", () => {
  it("sets data-theme and the browser chrome colour", () => {
    document.head.innerHTML = '<meta name="theme-color" content="#000000" />';
    applyTheme(document, "light");
    expect(document.documentElement.dataset.theme).toBe("light");
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      "content",
      THEME_COLOR.light,
    );
  });
});

/** Runs public/theme-boot.js against the current document, as the browser does before paint. */
function runBoot(): void {
  new Function(bootScript)();
}

describe("theme-boot.js agrees with resolveTheme", () => {
  afterEach(() => {
    window.localStorage.clear();
    delete document.documentElement.dataset.theme;
    vi.restoreAllMocks();
  });

  function bootWith(stored: string | null, prefersLight: boolean): string | undefined {
    document.head.innerHTML = '<meta name="theme-color" content="#000000" />';
    if (stored === null) window.localStorage.removeItem(THEME_STORAGE_KEY);
    else window.localStorage.setItem(THEME_STORAGE_KEY, stored);
    vi.spyOn(window, "matchMedia").mockImplementation(
      (query) => ({ matches: prefersLight && query.includes("light") }) as MediaQueryList,
    );
    runBoot();
    return document.documentElement.dataset.theme;
  }

  it("defaults to dark without a stored setting", () => {
    expect(bootWith(null, true)).toBe("dark");
    expect(resolveTheme("dark", true)).toBe("dark");
  });

  it.each<[ThemeSetting, boolean]>([
    ["dark", true],
    ["dark", false],
    ["light", true],
    ["light", false],
    ["system", true],
    ["system", false],
  ])("setting %s, system prefers light: %s", (setting, prefersLight) => {
    expect(bootWith(setting, prefersLight)).toBe(resolveTheme(setting, !prefersLight));
  });

  it("ignores garbage and keeps the dark default", () => {
    expect(bootWith("neon", false)).toBe("dark");
  });

  it("paints the browser chrome like applyTheme", () => {
    bootWith("light", false);
    expect(document.querySelector('meta[name="theme-color"]')).toHaveAttribute(
      "content",
      THEME_COLOR.light,
    );
  });

  it("falls back to dark when storage throws", () => {
    document.head.innerHTML = "";
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("blocked");
    });
    runBoot();
    expect(document.documentElement.dataset.theme).toBe("dark");
  });
});

describe("storeThemeSetting", () => {
  it("writes the setting under the key the boot script reads", () => {
    const set = vi.fn();
    storeThemeSetting({ setItem: set }, "light");
    expect(set).toHaveBeenCalledWith(THEME_STORAGE_KEY, "light");
  });

  it("carries on when storage is blocked", () => {
    expect(() =>
      storeThemeSetting(
        {
          setItem: () => {
            throw new DOMException("blocked", "SecurityError");
          },
        },
        "dark",
      ),
    ).not.toThrow();
  });
});
