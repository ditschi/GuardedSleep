<script lang="ts">
  import { getI18n } from "$lib/i18n/context.js";
  import { SETTINGS_TABS, type SettingsTab } from "$lib/settings/settings-tab.js";

  let {
    activeTab,
    onTabChange,
  }: {
    activeTab: SettingsTab;
    onTabChange: (tab: SettingsTab) => void;
  } = $props();

  const i18n = getI18n();

  const tabLabelKey: Record<SettingsTab, string> = {
    general: "settings.tabGeneral",
    child: "settings.tabChild",
    parent: "settings.tabParent",
  };
</script>

<div
  class="flex gap-1 rounded-xl border border-zinc-800 bg-zinc-900/50 p-1"
  role="tablist"
  aria-label={i18n?.t("settings.tablistAria") ?? "Settings sections"}
>
  {#each SETTINGS_TABS as tab (tab)}
    <button
      type="button"
      role="tab"
      id={`settings-tab-${tab}`}
      aria-selected={activeTab === tab}
      aria-controls={`settings-panel-${tab}`}
      data-testid={`settings-tab-${tab}`}
      class={`min-h-11 flex-1 rounded-lg px-2 py-2 text-sm font-medium transition-colors ${activeTab === tab ? "bg-emerald-800 text-emerald-50 shadow-sm" : "text-zinc-400 hover:bg-zinc-800/80 hover:text-zinc-200"}`}
      onclick={() => onTabChange(tab)}
    >
      {i18n?.t(tabLabelKey[tab]) ?? tab}
    </button>
  {/each}
</div>
