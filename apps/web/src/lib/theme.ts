/**
 * Theme resolution. `public/theme-boot.js` repeats `resolveTheme` in plain JavaScript because it
 * must run before first paint without a bundle; `theme.test.ts` runs the boot script and compares.
 */

/** What the user picks in settings (mirrors `Theme` in UiContract.kt). */
export type ThemeSetting = "dark" | "light" | "system";

/** What `<html data-theme>` carries; `dim` is the baby device while monitoring. */
export type AppliedTheme = "dark" | "light" | "dim";

/** The theme setting is not secret; this is the only key theme-boot.js reads. */
export const THEME_STORAGE_KEY = "guardedsleep.theme";

/** Browser chrome colour per theme (`<meta name="theme-color">`). */
export const THEME_COLOR: Record<AppliedTheme, string> = {
  dark: "#0e1013",
  light: "#f7f7f4",
  dim: "#000000",
};

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

export function applyTheme(doc: Document, theme: AppliedTheme): void {
  doc.documentElement.dataset.theme = theme;
  doc.querySelector('meta[name="theme-color"]')?.setAttribute("content", THEME_COLOR[theme]);
}
