<script lang="ts">
  import { browser } from "$app/environment";
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import "../app.css";
  import favicon from "$lib/assets/favicon.svg";
  import BackButton from "$lib/components/BackButton.svelte";
  import LanguageSelect from "$lib/components/LanguageSelect.svelte";
  import RuntimeCapabilityBanners from "$lib/components/RuntimeCapabilityBanners.svelte";
  import SettingsGearLink from "$lib/components/SettingsGearLink.svelte";
  import {
    readAppPreferences,
    applyAppTheme,
    resolveThemeDark,
    THEME_CHANGE_EVENT,
  } from "$lib/app-preferences.js";
  import { resolveBackFallback, shouldShowAppBack } from "$lib/ui/app-navigation.js";
  import { rawBundles } from "$lib/i18n/bundles";
  import { MASTER_LOCALE, STORAGE_KEY, type SupportedLocale } from "$lib/i18n/config";
  import { setI18nContext } from "$lib/i18n/context";
  import { deepMerge } from "$lib/i18n/merge-deep";
  import { formatMessage } from "$lib/i18n/icu";
  import { getByPath, hasLeafPath } from "$lib/i18n/paths";
  import { resolveClientLocale } from "$lib/i18n/resolve-client-locale";

  let { children } = $props();

  let locale = $state<SupportedLocale>(MASTER_LOCALE);
  let themeClass = $state("theme-dark");

  const showAppBack = $derived(shouldShowAppBack(page.url.pathname));
  const backFallbackHref = $derived(resolveBackFallback(page.url.pathname, page.url.searchParams));

  function applyTheme(): void {
    if (!browser) return;
    const dark = resolveThemeDark(readAppPreferences());
    themeClass = dark ? "theme-dark" : "theme-light";
    applyAppTheme();
  }

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
    return formatMessage(raw, params, locale);
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
    applyTheme();
    if (!browser) return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onMq = (): void => applyTheme();
    mq.addEventListener("change", onMq);
    const onStorage = (ev: StorageEvent): void => {
      if (ev.key === "guardedsleep:app-preferences") applyTheme();
    };
    const onThemeChange = (): void => applyTheme();
    window.addEventListener(THEME_CHANGE_EVENT, onThemeChange);
    window.addEventListener("storage", onStorage);
    return () => {
      mq.removeEventListener("change", onMq);
      window.removeEventListener(THEME_CHANGE_EVENT, onThemeChange);
      window.removeEventListener("storage", onStorage);
    };
  });
</script>

<svelte:head>
  <title>{t("meta.appTitle")}</title>
  <link rel="icon" href={favicon} />
</svelte:head>

<div
  class={`flex min-h-[100dvh] flex-col ${themeClass} ${themeClass === "theme-light" ? "bg-zinc-100 text-zinc-900" : "bg-zinc-950 text-zinc-100"}`}
>
  <RuntimeCapabilityBanners />
  <div
    class={`flex w-full min-w-0 shrink-0 flex-wrap items-center gap-2 border-b px-[max(0.5rem,env(safe-area-inset-left))] py-2 pr-[max(0.5rem,env(safe-area-inset-right))] pt-[max(0.5rem,env(safe-area-inset-top))] ${themeClass === "theme-light" ? "border-zinc-200 bg-white" : "border-zinc-800/80 bg-zinc-950"}`}
  >
    {#if showAppBack}
      <BackButton ariaLabel={t("layout.backAria")} fallbackHref={backFallbackHref} />
    {/if}
    <div class="ms-auto flex min-w-0 shrink flex-wrap items-center justify-end gap-2">
      <div class="min-w-0 shrink">
        <LanguageSelect />
      </div>
      <SettingsGearLink ariaLabel={t("layout.settingsAria")} />
    </div>
  </div>
  <div class="flex-1">
    {#key locale}
      {@render children()}
    {/key}
  </div>
</div>
