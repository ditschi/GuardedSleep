/**
 * Theme resolution. `public/theme-boot.js` repeats `resolveTheme` and `schemeFor` in plain
 * JavaScript because it must run before first paint without a bundle (together with `nightActive`
 * of `night-mode.ts`); `theme.test.ts` runs the boot script and compares.
 */

/** What the user picks in settings (mirrors `Theme` in UiContract.kt). */
export type ThemeSetting = "dark" | "light" | "system";

/** What `<html data-theme>` carries; `dim` is the baby device while monitoring. */
export type AppliedTheme = "dark" | "light" | "dim";

/** Colour scheme tokens (`<html data-scheme>`); Ember is night mode (`night-mode.ts`). */
export type ColorScheme = "dusk" | "day" | "ember";

/**
 * The scheme an applied theme uses: Dusk is the default, Day follows a light setting, and night
 * mode is Ember on top of either. Ember only recolours the scheme tokens; the `--safety-*` set
 * (alerts, status bar from level 1, warning banners, safety chips) is never scheme-dependent.
 */
export function schemeFor(theme: AppliedTheme, night = false): ColorScheme {
  if (night) return "ember";
  return theme === "light" ? "day" : "dusk";
}

/** The theme setting is not secret; this is the only key theme-boot.js reads. */
export const THEME_STORAGE_KEY = "guardedsleep.theme";

/** Browser chrome colour per theme (`<meta name="theme-color">`). */
export const THEME_COLOR: Record<AppliedTheme, string> = {
  dark: "#141218",
  light: "#faf7f4",
  dim: "#000000",
};

/** Browser chrome colour while night mode is on (Ember is black). */
export const NIGHT_THEME_COLOR = "#000000";

export function resolveTheme(
  setting: ThemeSetting,
  prefersDark: boolean,
  dimWhileMonitoring = false,
): AppliedTheme {
  if (dimWhileMonitoring) return "dim";
  if (setting === "system") return prefersDark ? "dark" : "light";
  return setting;
}

export function isThemeSetting(value: unknown): value is ThemeSetting {
  return value === "dark" || value === "light" || value === "system";
}

export function applyTheme(doc: Document, theme: AppliedTheme, night = false): void {
  doc.documentElement.dataset.theme = theme;
  doc.documentElement.dataset.scheme = schemeFor(theme, night);
  doc
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", night ? NIGHT_THEME_COLOR : THEME_COLOR[theme]);
}

/**
 * Remembers the theme setting for the next visit: `theme-boot.js` reads it before first paint.
 * Storage can be blocked; the setting then lasts for this page only.
 */
export function storeThemeSetting(storage: Pick<Storage, "setItem">, setting: ThemeSetting): void {
  try {
    storage.setItem(THEME_STORAGE_KEY, setting);
  } catch {
    // Private window or site data off: nothing to do.
  }
}

/** The remembered theme setting, or null when none (or an unknown value) is stored. */
export function readThemeSetting(storage: Pick<Storage, "getItem">): ThemeSetting | null {
  try {
    const value = storage.getItem(THEME_STORAGE_KEY);
    return isThemeSetting(value) ? value : null;
  } catch {
    return null;
  }
}
