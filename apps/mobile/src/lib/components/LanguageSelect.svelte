<script lang="ts">
  import { SUPPORTED_LOCALES, type SupportedLocale } from "$lib/i18n/config";
  import { getI18n } from "$lib/i18n/context";

  const i18n = getI18n()!;

  function onLanguageChange(e: Event): void {
    const v = (e.currentTarget as HTMLSelectElement).value;
    i18n.setLocale(v as SupportedLocale);
  }
</script>

<div class="flex min-w-0 max-w-full items-center">
  <label class="sr-only" for="gs-lang-select">{i18n.t("layout.languageSelectAria")}</label>
  <select
    id="gs-lang-select"
    class="h-11 min-w-0 max-w-[min(100vw-5.5rem,12rem)] rounded-lg border border-zinc-700 bg-zinc-900 px-2 text-sm text-zinc-200 focus:outline-none focus:ring-2 focus:ring-emerald-600/40"
    value={i18n.locale}
    onchange={onLanguageChange}
  >
    {#each SUPPORTED_LOCALES as loc (loc)}
      <option value={loc}>{i18n.t(`languages.${loc}`)}</option>
    {/each}
  </select>
</div>
