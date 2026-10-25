// `generated/*` is written by `pnpm run i18n:gen` from /locales (git-ignored); `dev`, `build`,
// `check` and `test` run it first. The shipped locale set, the endonyms and the bundles all come
// from there, so no list of languages is kept in this file.
import { ENDONYMS, GENERATED_BUNDLES, LOCALES, type Locale } from "./generated/locales";

export { ENDONYMS, LOCALES };
export type { Locale };

export const DEFAULT_LOCALE: Locale = "en";

/** A nested catalogue as generated: groups of groups of messages. */
export interface Bundle {
  readonly [key: string]: string | Bundle;
}

export const BUNDLES: Readonly<Record<Locale, Bundle>> = GENERATED_BUNDLES;

export function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && (LOCALES as readonly string[]).includes(value);
}

/** The shipped locale for a browser language tag: exact, else its base language (`de-AT` is `de`). */
function matchTag(tag: string): Locale | null {
  const lower = tag.toLowerCase();
  const exact = LOCALES.find((l) => l.toLowerCase() === lower);
  if (exact) return exact;
  const base = lower.split("-")[0];
  // A bare language also matches a shipped script variant (`zh` or `zh-CN` is `zh-Hans`).
  return LOCALES.find((l) => l.toLowerCase().split("-")[0] === base) ?? null;
}

/**
 * The locale to show: an explicit choice wins, otherwise the first browser language we ship,
 * otherwise English.
 */
export function resolveLocale(
  explicit: string | null,
  browserLanguages: readonly string[],
): Locale {
  if (isLocale(explicit)) return explicit;
  for (const tag of browserLanguages) {
    const match = matchTag(tag);
    if (match) return match;
  }
  return DEFAULT_LOCALE;
}

// Right-to-left languages among the shipped and plausible future locales. `Intl.Locale` text info
// is not in every browser yet, so a short list is the portable choice:
// https://developer.mozilla.org/docs/Web/JavaScript/Reference/Global_Objects/Intl/Locale/getTextInfo
const RTL_LANGUAGES: readonly string[] = ["ar", "fa", "he", "ur"];

/** Text direction of the UI language, for `<html dir>`. */
export function directionOf(locale: Locale): "ltr" | "rtl" {
  return RTL_LANGUAGES.includes(locale.split("-")[0] ?? locale) ? "rtl" : "ltr";
}
