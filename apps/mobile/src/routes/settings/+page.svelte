<script lang="ts">
  import { page } from "$app/state";
  import { isNativeAndroid } from "$lib/android/guarded-sleep-native.js";
  import { readBabyUnitPreferences } from "$lib/audio/baby-unit-preferences.js";
  import BabySoundDetectionSettings from "$lib/components/BabySoundDetectionSettings.svelte";
  import SignalingUrlEditor from "$lib/components/SignalingUrlEditor.svelte";
  import { getI18n } from "$lib/i18n/context.js";
  import { getEffectiveSignalingUrl } from "$lib/signaling-url.js";

  type SettingsTab = "general" | "child" | "parent";

  let activeTab = $state<SettingsTab>("general");

  let active = $state(getEffectiveSignalingUrl());

  const i18n = getI18n();

  const babyPrefs = readBabyUnitPreferences();
  let babyThreshold = $state(babyPrefs.threshold);
  let babyHoldMs = $state(babyPrefs.holdMs);
  let babyKiosk = $state(babyPrefs.kioskImmersive);

  function sync(): void {
    active = getEffectiveSignalingUrl();
  }

  const returnTo = $derived(page.url.searchParams.get("returnTo"));
  const returnRoom = $derived(page.url.searchParams.get("room") ?? "");
  const returnSignal = $derived(page.url.searchParams.get("signal") ?? "");

  const babyReturnHref = $derived.by(() => {
    if (returnTo !== "baby" || !returnRoom) return null;
    const q = new URLSearchParams({ room: returnRoom });
    if (returnSignal) q.set("signal", returnSignal);
    return `/baby?${q}`;
  });

  const tabs = $derived([
    { id: "general" as const, label: i18n?.t("settings.tabGeneral") ?? "" },
    { id: "child" as const, label: i18n?.t("settings.tabChild") ?? "" },
    { id: "parent" as const, label: i18n?.t("settings.tabParent") ?? "" },
  ]);
</script>

<div class="mx-auto flex max-w-md flex-col gap-6 px-4 py-8">
  <header class="flex items-center justify-between gap-2">
    <h1 class="text-xl font-semibold text-zinc-50">{i18n?.t("settings.title") ?? ""}</h1>
    <a class="text-sm text-emerald-400 underline hover:text-emerald-300" href="/"
      >{i18n?.t("settings.homeLink") ?? ""}</a
    >
  </header>

  {#if babyReturnHref}
    <div
      class="rounded-xl border border-emerald-800/50 bg-emerald-950/25 p-4 text-sm text-zinc-300"
    >
      <p class="font-medium text-emerald-200/90">{i18n?.t("settings.returnBabyTitle") ?? ""}</p>
      <p class="mt-1 text-xs text-zinc-500">{i18n?.t("settings.returnBabyHelp") ?? ""}</p>
      <a
        class="mt-3 inline-flex rounded-lg bg-emerald-700 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-600"
        href={babyReturnHref}
      >
        {i18n?.t("settings.returnBabyButton") ?? ""}
      </a>
    </div>
  {/if}

  <div class="rounded-xl border border-zinc-800 bg-zinc-900/40 p-1">
    <div
      class="grid grid-cols-3 gap-1"
      role="tablist"
      aria-label={i18n?.t("settings.tablistAria") ?? ""}
    >
      {#each tabs as t (t.id)}
        <button
          type="button"
          role="tab"
          id="settings-tab-{t.id}"
          class="rounded-lg px-2 py-2 text-center text-xs font-medium sm:text-sm {activeTab === t.id
            ? 'bg-zinc-800 text-zinc-50 shadow-sm ring-1 ring-zinc-700/80'
            : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'}"
          aria-selected={activeTab === t.id}
          aria-controls="settings-panel-{t.id}"
          tabindex={activeTab === t.id ? 0 : -1}
          onclick={() => {
            activeTab = t.id;
          }}
        >
          {t.label}
        </button>
      {/each}
    </div>
  </div>

  {#if activeTab === "general"}
    <div
      id="settings-panel-general"
      class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4"
      role="tabpanel"
      aria-labelledby="settings-tab-general"
    >
      <h2 class="text-sm font-semibold text-zinc-100">{i18n?.t("settings.generalTitle") ?? ""}</h2>
      <p class="text-sm text-zinc-500">{i18n?.t("settings.generalIntro") ?? ""}</p>
      <div class="space-y-2 rounded-lg border border-zinc-800/80 bg-zinc-950/50 p-3">
        <h3 class="text-xs font-medium uppercase tracking-wide text-zinc-500">
          {i18n?.t("settings.signalingHeading") ?? ""}
        </h3>
        <p class="text-sm text-zinc-500">{i18n?.t("settings.signalingHelp") ?? ""}</p>
        <SignalingUrlEditor onChanged={sync} />
        <p class="text-xs text-zinc-600">
          {i18n?.t("settings.activeDefault") ?? ""}
          <span class="font-mono text-zinc-500">{active}</span>
        </p>
      </div>
    </div>
  {:else if activeTab === "child"}
    <div
      id="settings-panel-child"
      class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4"
      role="tabpanel"
      aria-labelledby="settings-tab-child"
    >
      <h2 class="text-sm font-semibold text-zinc-100">{i18n?.t("settings.childTitle") ?? ""}</h2>
      <p class="text-sm text-zinc-500">{i18n?.t("settings.childIntro1") ?? ""}</p>
      <p class="text-sm text-zinc-500">{i18n?.t("settings.childIntro2") ?? ""}</p>
      <BabySoundDetectionSettings
        showKiosk={isNativeAndroid()}
        bind:threshold={babyThreshold}
        bind:holdMs={babyHoldMs}
        bind:kioskImmersive={babyKiosk}
      />
      <a
        class="inline-flex text-sm font-medium text-emerald-400 underline hover:text-emerald-300"
        href="/pair?role=baby"
      >
        {i18n?.t("settings.childPairingLink") ?? ""}
      </a>
    </div>
  {:else}
    <div
      id="settings-panel-parent"
      class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4"
      role="tabpanel"
      aria-labelledby="settings-tab-parent"
    >
      <h2 class="text-sm font-semibold text-zinc-100">{i18n?.t("settings.parentTitle") ?? ""}</h2>
      <p class="text-sm text-zinc-500">{i18n?.t("settings.parentIntro") ?? ""}</p>
      <a
        class="inline-flex text-sm font-medium text-emerald-400 underline hover:text-emerald-300"
        href="/pair?role=parent"
      >
        {i18n?.t("settings.parentPairingLink") ?? ""}
      </a>
    </div>
  {/if}
</div>
