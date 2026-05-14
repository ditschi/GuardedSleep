<script lang="ts">
  import {
    formatHoldDurationLabel,
    HOLD_DURATION_CUSTOM_MAX_MS,
    HOLD_DURATION_CUSTOM_MIN_MS,
    HOLD_DURATION_DEFAULT_MS,
    HOLD_DURATION_PRESET_MS,
  } from "$lib/audio/hold-duration-presets.js";
  import {
    persistBabyKioskImmersive,
    persistBabyNoiseGate,
  } from "$lib/audio/baby-unit-preferences.js";
  import { getI18n } from "$lib/i18n/context.js";

  let {
    showKiosk = false,
    threshold = $bindable(0.035),
    holdMs = $bindable(HOLD_DURATION_DEFAULT_MS),
    kioskImmersive = $bindable(true),
  }: {
    showKiosk?: boolean;
    threshold?: number;
    holdMs?: number;
    kioskImmersive?: boolean;
  } = $props();

  const i18n = getI18n();

  let holdCustomOpen = $state(false);

  $effect(() => {
    persistBabyNoiseGate(threshold, holdMs);
  });

  $effect(() => {
    if (showKiosk) persistBabyKioskImmersive(kioskImmersive);
  });
</script>

<div class="space-y-4 text-sm text-zinc-400">
  {#if showKiosk}
    <label class="flex items-center gap-2">
      <input type="checkbox" bind:checked={kioskImmersive} class="min-h-11 min-w-11" />
      <span>{i18n?.t("babySound.kioskLabel") ?? ""}</span>
    </label>
  {/if}

  <div class="rounded-lg border border-zinc-800 bg-zinc-950/80 p-3">
    <p class="mb-1 text-xs uppercase tracking-wide text-zinc-500">
      {i18n?.t("babySound.sectionTitle") ?? ""}
    </p>
    <label class="flex flex-col gap-1">
      <span class="text-xs text-zinc-300"
        >{i18n?.t("babySound.sensitivityLabel", { value: threshold.toFixed(3) }) ?? ""}</span
      >
      <input
        type="range"
        min="0.01"
        max="0.15"
        step="0.005"
        bind:value={threshold}
        class="w-full min-h-11"
      />
    </label>
    <div class="mt-4 border-t border-zinc-800 pt-3">
      <p class="text-xs font-medium text-zinc-300">{i18n?.t("babySound.holdTitle") ?? ""}</p>
      <p class="mb-3 text-xs text-zinc-500">{i18n?.t("babySound.holdHelp") ?? ""}</p>
      <div class="grid grid-cols-2 gap-2 sm:grid-cols-4">
        {#each HOLD_DURATION_PRESET_MS as ms (ms)}
          <button
            type="button"
            class={`min-h-11 rounded-lg border px-2 py-2 text-sm font-medium transition-colors ${holdMs === ms ? "border-emerald-500 bg-emerald-950/50 text-emerald-200" : "border-zinc-700 bg-zinc-900/40 text-zinc-200 hover:border-zinc-600"}`}
            onclick={() => {
              holdMs = ms;
              holdCustomOpen = false;
            }}
          >
            {formatHoldDurationLabel(ms)}
          </button>
        {/each}
      </div>
      <details
        class="mt-3 rounded-lg border border-zinc-800 bg-zinc-900/30"
        bind:open={holdCustomOpen}
      >
        <summary
          class="cursor-pointer select-none px-3 py-3 text-sm text-zinc-300 marker:text-zinc-500"
        >
          {i18n?.t("babySound.customSummary") ?? ""}
        </summary>
        <div class="border-t border-zinc-800 px-3 pb-3 pt-1">
          <label class="flex flex-col gap-2">
            <span class="text-xs text-zinc-400"
              >{i18n?.t("babySound.selected", { label: formatHoldDurationLabel(holdMs) }) ??
                ""}</span
            >
            <input
              type="range"
              min={HOLD_DURATION_CUSTOM_MIN_MS}
              max={HOLD_DURATION_CUSTOM_MAX_MS}
              step="100"
              bind:value={holdMs}
              class="w-full min-h-11"
            />
            <span class="text-[11px] text-zinc-600">
              {i18n?.t("babySound.customRange", {
                min: HOLD_DURATION_CUSTOM_MIN_MS / 1000,
                max: HOLD_DURATION_CUSTOM_MAX_MS / 1000,
              }) ?? ""}
            </span>
          </label>
        </div>
      </details>
    </div>
  </div>
</div>
