import { derived, writable, type Readable } from "svelte/store";
import { DEFAULT_LOCALE, type Locale } from "./bundles";
import { createTranslator, type Translate } from "./translate";

/** The active UI locale; `SettingsState.language` and the browser languages feed it. */
export const locale = writable<Locale>(DEFAULT_LOCALE);

/** `$t("group.key", { arg })` in components; re-evaluates when the locale changes. */
export const t: Readable<Translate> = derived(locale, ($locale) => createTranslator($locale));
