<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import favicon from "$lib/assets/favicon.svg";
  import LanguageSelect from "$lib/components/LanguageSelect.svelte";
  import RuntimeCapabilityBanners from "$lib/components/RuntimeCapabilityBanners.svelte";
  import SettingsGearLink from "$lib/components/SettingsGearLink.svelte";
  import { rawBundles } from "$lib/i18n/bundles";
  import { MASTER_LOCALE, STORAGE_KEY, type SupportedLocale } from "$lib/i18n/config";
  import { setI18nContext } from "$lib/i18n/context";
  import { deepMerge } from "$lib/i18n/merge-deep";
  import { getByPath, hasLeafPath, interpolate } from "$lib/i18n/paths";
  import { resolveClientLocale } from "$lib/i18n/resolve-client-locale";

  let { children } = $props();

  let locale = $state<SupportedLocale>(MASTER_LOCALE);

  function mergedFor(loc: SupportedLocale): Record<string, unknown> {
    const base = rawBundles[MASTER_LOCALE];
    if (loc === MASTER_LOCALE) {
      return deepMerge({}, base);
    }
    return deepMerge(deepMerge({}, base), rawBundles[loc]);
  }

  function t(key: string, params?: Record<string, string | number>): string {
    locale;
    if (import.meta.env.DEV && locale !== MASTER_LOCALE) {
      const overlay = rawBundles[locale];
      if (hasLeafPath(rawBundles[MASTER_LOCALE], key) && !hasLeafPath(overlay, key)) {
        console.warn(`[i18n] Missing "${key}" for locale "${locale}", using ${MASTER_LOCALE}`);
      }
    }
    const merged = mergedFor(locale);
    const raw = getByPath(merged, key);
    if (typeof raw !== "string" || raw === "") {
      if (import.meta.env.DEV) {
        console.warn(`[i18n] Missing key "${key}" in merged bundle`);
      }
      return key;
    }
    return interpolate(raw, params);
  }

  function setLocale(next: SupportedLocale): void {
    locale = next;
    try {
      localStorage.setItem(STORAGE_KEY, next);
    } catch {
      /* private mode, etc. */
    }
  }

  setI18nContext({
    get locale() {
      return locale;
    },
    setLocale,
    t,
  });

  onMount(() => {
    const next = resolveClientLocale();
    if (next !== locale) {
      locale = next;
    }
  });
</script>

<svelte:head>
  <title>{t("meta.appTitle")}</title>
  <link rel="icon" href={favicon} />
</svelte:head>

<div class="flex min-h-[100dvh] flex-col bg-zinc-950 text-zinc-100">
  <RuntimeCapabilityBanners />
  <div
    class="flex w-full min-w-0 shrink-0 flex-wrap items-center justify-end gap-2 border-b border-zinc-800/80 bg-zinc-950 px-[max(0.5rem,env(safe-area-inset-left))] py-2 pr-[max(0.5rem,env(safe-area-inset-right))] pt-[max(0.5rem,env(safe-area-inset-top))]"
  >
    <div class="min-w-0 shrink">
      <LanguageSelect />
    </div>
    <SettingsGearLink ariaLabel={t("layout.settingsAria")} />
  </div>
  <div class="flex-1">
    {#key locale}
      {@render children()}
    {/key}
  </div>
</div>
