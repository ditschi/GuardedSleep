export const MASTER_LOCALE = "en" as const;

export const SUPPORTED_LOCALES = ["en", "de", "fr", "es"] as const;

export type SupportedLocale = (typeof SUPPORTED_LOCALES)[number];

export const STORAGE_KEY = "guardedsleep.locale";

export function isSupportedLocale(value: string): value is SupportedLocale {
  return (SUPPORTED_LOCALES as readonly string[]).includes(value);
}

/** e.g. de-DE, de → de; en-US → en */
export function normalizeLanguageTag(tag: string): string | null {
  const t = tag.trim().toLowerCase();
  if (!t) return null;
  const base = t.split("-")[0] ?? t;
  return /^[a-z]{2}$/.test(base) ? base : null;
}
