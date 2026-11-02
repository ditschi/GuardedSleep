import { BUNDLES, DEFAULT_LOCALE, type Bundle, type Locale } from "./bundles";
import { formatMessage, type MessageArgs } from "./icu";
import type { MessageKey } from "./keys";

export type Translate = (key: MessageKey, args?: MessageArgs) => string;

function lookup(bundle: Bundle, key: string): string | undefined {
  let node: string | Bundle | undefined = bundle;
  for (const part of key.split(".")) {
    if (node === undefined || typeof node === "string") return undefined;
    node = node[part];
  }
  return typeof node === "string" ? node : undefined;
}

export interface TranslatorOptions {
  /** Test seam: replaces the generated bundles. */
  bundles?: Readonly<Record<string, Bundle>>;
  /**
   * Keys that must never show another language (alerts, link loss, connection, safety;
   * AGENTS.md section 5.2a). The generator only emits a locale when it has all of them, so a
   * miss here is a build error elsewhere; the key is shown rather than a mixed-language alarm.
   * The catalogue does not tag critical keys yet (`expand-locales`), so the default set is empty.
   */
  criticalKeys?: ReadonlySet<string>;
}

/**
 * `t(key, args)` for one locale: ICU formatting, and English for a non-critical key the locale
 * does not have. An unknown key (only reachable through a cast) returns the key itself.
 */
export function createTranslator(locale: Locale, options: TranslatorOptions = {}): Translate {
  const bundles = options.bundles ?? BUNDLES;
  const critical = options.criticalKeys ?? new Set<string>();
  const own = bundles[locale];
  const master = bundles[DEFAULT_LOCALE];
  return (key, args) => {
    const hit = own ? lookup(own, key) : undefined;
    if (hit !== undefined) return formatMessage(hit, args, locale);
    if (!critical.has(key) && master) {
      const fallback = lookup(master, key);
      if (fallback !== undefined) return formatMessage(fallback, args, DEFAULT_LOCALE);
    }
    return key;
  };
}
