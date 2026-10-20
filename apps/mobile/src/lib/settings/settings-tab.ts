export type SettingsTab = "general" | "child" | "parent";

export const SETTINGS_TABS: readonly SettingsTab[] = ["general", "child", "parent"];

export function parseSettingsTab(raw: string | null | undefined): SettingsTab | null {
  if (raw === "general" || raw === "child" || raw === "parent") return raw;
  return null;
}

export function defaultSettingsTab(returnTo: string | null | undefined): SettingsTab {
  if (returnTo === "baby") return "child";
  if (returnTo === "parent") return "parent";
  return "general";
}

export function resolveSettingsTab(
  tabParam: string | null | undefined,
  returnTo: string | null | undefined,
): SettingsTab {
  return parseSettingsTab(tabParam) ?? defaultSettingsTab(returnTo);
}
