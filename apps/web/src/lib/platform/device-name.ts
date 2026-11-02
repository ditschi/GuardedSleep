/**
 * The name this browser shows on the baby device ("Connected to Chrome on Android"): the browser
 * family and the operating system, nothing finer. No version, no model, no language and no id, so
 * it identifies a kind of device to the person who owns it and fingerprints nobody. It travels to
 * the baby device over the verified DataChannel only (`LinkConfig.device_name`).
 */

interface NavigatorLike {
  userAgent: string;
  userAgentData?: { platform?: string };
}

/** Order matters: Edge, Opera and Samsung Internet also say "Chrome" in their user agent. */
const BROWSERS: [RegExp, string][] = [
  [/Edg(?:e|A|iOS)?\//, "Edge"],
  [/OPR\/|Opera/, "Opera"],
  [/SamsungBrowser\//, "Samsung Internet"],
  [/Firefox\/|FxiOS\//, "Firefox"],
  [/Chrome\/|CriOS\//, "Chrome"],
  [/Safari\//, "Safari"],
];

const SYSTEMS: [RegExp, string][] = [
  [/Android/, "Android"],
  [/iPhone|iPad|iPod/, "iOS"],
  [/CrOS/, "ChromeOS"],
  [/Windows/, "Windows"],
  [/Mac OS X|Macintosh|macOS/, "macOS"],
  [/Linux|X11/, "Linux"],
];

function firstMatch(table: [RegExp, string][], text: string): string | null {
  for (const [pattern, name] of table) {
    if (pattern.test(text)) return name;
  }
  return null;
}

export function browserDeviceName(nav: NavigatorLike): string {
  const browser = firstMatch(BROWSERS, nav.userAgent) ?? "Browser";
  const system =
    firstMatch(SYSTEMS, nav.userAgentData?.platform ?? "") ?? firstMatch(SYSTEMS, nav.userAgent);
  return system === null ? browser : `${browser} on ${system}`;
}
