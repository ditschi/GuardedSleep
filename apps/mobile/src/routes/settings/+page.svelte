<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { readBabyUnitPreferences } from "$lib/audio/baby-unit-preferences.js";
  import {
    HEARTBEAT_INTERVAL_CHOICES,
    applyAppTheme,
    persistAppPreferences,
    readAppPreferences,
    type AlertAudioOutput,
    type NightModePreference,
  } from "$lib/app-preferences.js";
  import BabySoundDetectionSettings from "$lib/components/BabySoundDetectionSettings.svelte";
  import SettingsTabBar from "$lib/components/SettingsTabBar.svelte";
  import SignalingUrlEditor from "$lib/components/SignalingUrlEditor.svelte";
  import { getI18n } from "$lib/i18n/context.js";
  import { getEffectiveSignalingUrl } from "$lib/signaling-url.js";
  import { resolveSettingsTab, type SettingsTab } from "$lib/settings/settings-tab.js";

  const i18n = getI18n();

  let prefs = $state(readAppPreferences());
  let activeSignalingUrl = $state(getEffectiveSignalingUrl());

  const babyPrefs = readBabyUnitPreferences();
  let babyThreshold = $state(babyPrefs.threshold);
  let babyHoldMs = $state(babyPrefs.holdMs);

  const returnTo = $derived(page.url.searchParams.get("returnTo"));

  const activeTab = $derived(resolveSettingsTab(page.url.searchParams.get("tab"), returnTo));

  function patchPrefs(patch: Parameters<typeof persistAppPreferences>[0]): void {
    prefs = persistAppPreferences(patch);
    if (patch.nightMode != null) applyAppTheme();
  }

  function syncSignaling(): void {
    activeSignalingUrl = getEffectiveSignalingUrl();
  }

  function setTab(tab: SettingsTab): void {
    const q = new URLSearchParams(page.url.searchParams);
    q.set("tab", tab);
    void goto(`/settings?${q}`, { replaceState: true, keepFocus: true, noScroll: true });
  }

  const nightModeOptions: { value: NightModePreference; labelKey: string }[] = [
    { value: "system", labelKey: "settings.displayNightSystem" },
    { value: "light", labelKey: "settings.displayNightLight" },
    { value: "dark", labelKey: "settings.displayNightDark" },
  ];

  const alertOutputOptions: { value: AlertAudioOutput; labelKey: string }[] = [
    { value: "default", labelKey: "settings.alertOutputDefault" },
    { value: "speaker", labelKey: "settings.alertOutputSpeaker" },
    { value: "speaker_default", labelKey: "settings.alertOutputSpeakerDefault" },
  ];
</script>

<div class="mx-auto flex max-w-md flex-col gap-6 px-4 py-8">
  <header>
    <h1 class="text-xl font-semibold text-zinc-50">
      {i18n?.t("settings.title") ?? ""}
    </h1>
  </header>

  <SettingsTabBar {activeTab} onTabChange={setTab} />

  {#if activeTab === "general"}
    <div
      id="settings-panel-general"
      role="tabpanel"
      aria-labelledby="settings-tab-general"
      class="flex flex-col gap-6"
    >
      <p class="text-sm text-zinc-500">{i18n?.t("settings.generalIntro") ?? ""}</p>

      <section class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4">
        <h2 class="text-sm font-semibold text-zinc-100">
          {i18n?.t("settings.sectionDisplay") ?? ""}
        </h2>
        <label class="flex flex-col gap-1 text-sm">
          <span class="text-zinc-400">{i18n?.t("settings.displayNightLabel") ?? ""}</span>
          <select
            class="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100"
            value={prefs.nightMode}
            onchange={(e) => {
              patchPrefs({ nightMode: e.currentTarget.value as NightModePreference });
            }}
          >
            {#each nightModeOptions as opt (opt.value)}
              <option value={opt.value}>{i18n?.t(opt.labelKey) ?? opt.value}</option>
            {/each}
          </select>
        </label>
        <label class="flex items-center gap-2 text-sm text-zinc-300">
          <input
            type="checkbox"
            class="min-h-11 min-w-11"
            checked={prefs.showConnectionDetails}
            onchange={(e) => {
              patchPrefs({ showConnectionDetails: e.currentTarget.checked });
            }}
          />
          <span>{i18n?.t("settings.showConnectionDetailsLabel") ?? ""}</span>
        </label>
      </section>

      <section class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4">
        <h2 class="text-sm font-semibold text-zinc-100">
          {i18n?.t("settings.sectionAbout") ?? ""}
        </h2>
        <p class="text-sm text-zinc-500">{i18n?.t("settings.aboutIntro") ?? ""}</p>
        <button
          type="button"
          class="text-sm text-emerald-400 underline hover:text-emerald-300"
          onclick={() => goto("/onboard")}
        >
          {i18n?.t("settings.replayOnboarding") ?? ""}
        </button>
      </section>

      <div class="rounded-xl border border-zinc-800 bg-zinc-900/30 p-4">
        <button
          type="button"
          class="flex w-full items-center justify-between gap-2 text-sm font-medium text-zinc-200"
          data-testid="settings-advanced-toggle"
          aria-expanded={prefs.showAdvancedSettings}
          onclick={() => patchPrefs({ showAdvancedSettings: !prefs.showAdvancedSettings })}
        >
          <span>
            {prefs.showAdvancedSettings
              ? (i18n?.t("settings.hideAdvanced") ?? "")
              : (i18n?.t("settings.showAdvanced") ?? "")}
          </span>
          <span aria-hidden="true">{prefs.showAdvancedSettings ? "▲" : "▼"}</span>
        </button>
      </div>

      {#if prefs.showAdvancedSettings}
        <div
          class="space-y-4 rounded-xl border border-amber-900/50 bg-amber-950/20 p-4"
          data-testid="settings-advanced-panel"
        >
          <p class="text-sm text-amber-100/90">{i18n?.t("settings.advancedWarning") ?? ""}</p>

          <div class="space-y-2">
            <h3 class="text-xs font-semibold uppercase tracking-wide text-zinc-500">
              {i18n?.t("settings.signalingHeading") ?? ""}
            </h3>
            <p class="text-xs text-zinc-500">{i18n?.t("settings.signalingHelp") ?? ""}</p>
            <SignalingUrlEditor onChanged={syncSignaling} />
            <p class="text-xs text-zinc-600">
              {i18n?.t("settings.activeDefault") ?? ""}
              <span class="font-mono text-zinc-500">{activeSignalingUrl}</span>
            </p>
          </div>

          <label class="flex flex-col gap-1 text-sm">
            <span class="text-zinc-400">{i18n?.t("settings.heartbeatLabel") ?? ""}</span>
            <select
              class="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100"
              value={prefs.heartbeatIntervalSec}
              onchange={(e) => {
                patchPrefs({ heartbeatIntervalSec: Number(e.currentTarget.value) });
              }}
            >
              {#each HEARTBEAT_INTERVAL_CHOICES as sec (sec)}
                <option value={sec}
                  >{i18n?.t("settings.heartbeatOption", { seconds: sec }) ?? ""}</option
                >
              {/each}
            </select>
          </label>

          <p class="text-xs text-zinc-600">{i18n?.t("settings.diagnosticsPlaceholder") ?? ""}</p>
        </div>
      {/if}
    </div>
  {:else if activeTab === "child"}
    <div
      id="settings-panel-child"
      role="tabpanel"
      aria-labelledby="settings-tab-child"
      class="flex flex-col gap-6"
    >
      <div class="space-y-1">
        <h2 class="text-sm font-semibold text-zinc-100">{i18n?.t("settings.childTitle") ?? ""}</h2>
        <p class="text-sm text-zinc-500">{i18n?.t("settings.childIntro1") ?? ""}</p>
        <p class="text-xs text-zinc-600">{i18n?.t("settings.childIntro2") ?? ""}</p>
      </div>

      <section class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4">
        <h3 class="text-sm font-semibold text-zinc-100">
          {i18n?.t("settings.sectionBaby") ?? ""}
        </h3>
        <BabySoundDetectionSettings bind:threshold={babyThreshold} bind:holdMs={babyHoldMs} />
        <a
          class="inline-flex text-sm font-medium text-emerald-400 underline hover:text-emerald-300"
          href="/pair?role=baby"
        >
          {i18n?.t("settings.childPairingLink") ?? ""}
        </a>
      </section>

      <section class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4">
        <h3 class="text-sm font-semibold text-zinc-100">
          {i18n?.t("settings.sectionBattery") ?? ""}
        </h3>
        <p class="text-sm text-zinc-500">{i18n?.t("settings.batteryIntro") ?? ""}</p>
        <p class="text-xs text-zinc-600">{i18n?.t("settings.batteryWebHint") ?? ""}</p>
      </section>
    </div>
  {:else}
    <div
      id="settings-panel-parent"
      role="tabpanel"
      aria-labelledby="settings-tab-parent"
      class="flex flex-col gap-6"
    >
      <div class="space-y-1">
        <h2 class="text-sm font-semibold text-zinc-100">{i18n?.t("settings.parentTitle") ?? ""}</h2>
        <p class="text-sm text-zinc-500">{i18n?.t("settings.parentIntro") ?? ""}</p>
      </div>

      <section class="space-y-3 rounded-xl border border-zinc-800 bg-zinc-900/40 p-4">
        <h3 class="text-sm font-semibold text-zinc-100">
          {i18n?.t("settings.sectionNotifications") ?? ""}
        </h3>
        <p class="text-xs text-zinc-500">{i18n?.t("settings.notificationsIntro") ?? ""}</p>
        <label class="flex flex-col gap-1 text-sm">
          <span class="text-zinc-400">{i18n?.t("settings.alertOutputLabel") ?? ""}</span>
          <select
            class="rounded-lg border border-zinc-700 bg-zinc-950 px-3 py-2 text-zinc-100"
            value={prefs.alertAudioOutput}
            onchange={(e) => {
              patchPrefs({ alertAudioOutput: e.currentTarget.value as AlertAudioOutput });
            }}
          >
            {#each alertOutputOptions as opt (opt.value)}
              <option value={opt.value}>{i18n?.t(opt.labelKey) ?? opt.value}</option>
            {/each}
          </select>
        </label>
        <label class="flex flex-col gap-2 text-sm">
          <span class="text-zinc-400"
            >{i18n?.t("settings.alertGainLabel", { value: prefs.alertPlaybackGain.toFixed(1) }) ??
              ""}</span
          >
          <input
            type="range"
            min="0.5"
            max="2"
            step="0.1"
            value={prefs.alertPlaybackGain}
            class="min-h-11 w-full"
            oninput={(e) => {
              patchPrefs({ alertPlaybackGain: Number(e.currentTarget.value) });
            }}
          />
        </label>
        <label class="flex items-center gap-2 text-sm text-zinc-300">
          <input
            type="checkbox"
            class="min-h-11 min-w-11"
            checked={prefs.alertMinGainDuringAlert}
            onchange={(e) => {
              patchPrefs({ alertMinGainDuringAlert: e.currentTarget.checked });
            }}
          />
          <span>{i18n?.t("settings.alertMinGainLabel") ?? ""}</span>
        </label>
        <label class="flex items-center gap-2 text-sm text-zinc-300">
          <input
            type="checkbox"
            class="min-h-11 min-w-11"
            checked={prefs.wakeLockInListenMode}
            onchange={(e) => {
              patchPrefs({ wakeLockInListenMode: e.currentTarget.checked });
            }}
          />
          <span>{i18n?.t("settings.wakeLockListenLabel") ?? ""}</span>
        </label>
      </section>

      <a
        class="inline-flex text-sm font-medium text-emerald-400 underline hover:text-emerald-300"
        href="/pair?role=parent"
      >
        {i18n?.t("settings.parentPairingLink") ?? ""}
      </a>
    </div>
  {/if}
</div>
