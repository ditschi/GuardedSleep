import {
  isSupportedLocale,
  MASTER_LOCALE,
  normalizeLanguageTag,
  STORAGE_KEY,
  type SupportedLocale,
} from "./config";

export function readStoredLocale(): SupportedLocale | null {
  if (typeof localStorage === "undefined") return null;
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw && isSupportedLocale(raw)) return raw;
  } catch {
    /* ignore */
  }
  return null;
}

export function readNavigatorLocale(): SupportedLocale | null {
  if (typeof navigator === "undefined") return null;
  const list =
    navigator.languages && navigator.languages.length > 0
      ? navigator.languages
      : [navigator.language];
  for (const tag of list) {
    const n = normalizeLanguageTag(tag);
    if (n && isSupportedLocale(n)) return n;
  }
  return null;
}

export function resolveClientLocale(): SupportedLocale {
  return readStoredLocale() ?? readNavigatorLocale() ?? MASTER_LOCALE;
}
