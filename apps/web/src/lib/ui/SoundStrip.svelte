<script lang="ts">
  import { untrack } from "svelte";
  import { t } from "../i18n/store";
  import { LevelHistory } from "./level-history";
  import { meterFraction } from "./sensitivity";
  import { everySecond } from "./ticker";

  /**
   * The monitor's sound strip (`parent-alerts` "Monitor sound strip"): the last minute of the live
   * level as 60 one-second bars, a dashed line where the baby device's alert threshold is, and a
   * caption row. Presentation only: the history is sampled once per second while connected and no
   * alert decision reads it. Not connected, the bars are flat; with a silent microphone the strip is
   * hatched, because silence and a dead microphone must never look alike. Its accessible name is
   * the level in words; it is no live region.
   */
  type Props = {
    /** Live level 0..1. */
    level: number;
    connected: boolean;
    /** The baby device's noise threshold 0..1, drawn as the dashed line when known. */
    threshold: number | null;
    micSilent: boolean;
    /** Accessible name ("Sound level: quiet"). */
    label: string;
    testid?: string;
  };

  let { level, connected, threshold, micSilent, label, testid }: Props = $props();

  const history = new LevelHistory();
  let bars = $state<number[]>(new Array(60).fill(0));

  // One sample per tick of the shared one-second ticker; the level itself is read untracked, so a
  // faster-moving level never adds samples.
  $effect(() => {
    const now = $everySecond.elapsedMs;
    if (connected) untrack(() => history.push(now, level));
    else history.clear();
    bars = history.bars(now);
  });

  // Bars and line share the slider's decibel scale (`meterFraction`); "above" compares raw levels.
  const line = $derived(threshold === null ? null : meterFraction(threshold));
</script>

<div class="gs-strip-block">
  <div
    class="gs-strip"
    data-testid={testid}
    data-state={!connected ? "offline" : micSilent ? "mic_silent" : "live"}
    role="meter"
    aria-valuemin="0"
    aria-valuemax="1"
    aria-valuenow={Math.min(1, Math.max(0, connected ? level : 0))}
    aria-valuetext={label}
    aria-label={label}
  >
    <div class="gs-strip__bars" aria-hidden="true">
      {#each bars as bar, i (i)}
        <span
          class="gs-strip__bar"
          data-above={threshold !== null && bar >= threshold && bar > 0 ? "" : undefined}
          style:height={`${Math.max(2, Math.round(meterFraction(bar) * 100))}%`}
        ></span>
      {/each}
      {#if line !== null && connected && !micSilent}
        <span class="gs-strip__line" style:bottom={`${line * 100}%`}></span>
      {/if}
    </div>
    {#if !connected}
      <span class="gs-strip__center" aria-hidden="true">{$t("native.ui.monitor.noSound")}</span>
    {:else if micSilent}
      <span class="gs-strip__center" aria-hidden="true">
        {$t("native.ui.monitor.micSilentStrip")}
      </span>
    {/if}
  </div>
  <div class="gs-strip__captions" aria-hidden="true">
    <span>{$t("native.ui.monitor.lastMinute")}</span>
    {#if line !== null && connected && !micSilent}
      <span class="gs-strip__line-label">{$t("native.ui.monitor.alertLine")}</span>
    {/if}
    <span>{$t("native.ui.monitor.now")}</span>
  </div>
</div>
