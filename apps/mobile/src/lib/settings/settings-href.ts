import type { SettingsTab } from "./settings-tab.js";

/** Build /settings URL with optional session return context from baby or parent routes. */
export function buildSettingsHref(opts: {
  pathname: string;
  searchParams: URLSearchParams;
  tab?: SettingsTab;
}): string {
  const { pathname, searchParams } = opts;
  const q = new URLSearchParams();

  let tab = opts.tab;
  if (pathname === "/baby") {
    q.set("returnTo", "baby");
    const room = searchParams.get("room");
    const signal = searchParams.get("signal");
    if (room) q.set("room", room);
    if (signal) q.set("signal", signal);
    tab ??= "child";
  } else if (pathname === "/parent") {
    q.set("returnTo", "parent");
    const room = searchParams.get("room");
    const signal = searchParams.get("signal");
    if (room) q.set("room", room);
    if (signal) q.set("signal", signal);
    tab ??= "parent";
  } else {
    tab ??= "general";
  }

  q.set("tab", tab);
  return `/settings?${q}`;
}
