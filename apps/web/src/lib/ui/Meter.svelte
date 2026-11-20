<script lang="ts">
  import { meterBars } from "./meter";
  import { meterFraction } from "./sensitivity";

  type Props = {
    /** Live level 0..1. */
    level: number;
    /** The baby device's noise threshold 0..1, drawn as a marker line when known. */
    threshold?: number | null;
    /** Accessible text for the level ("Quiet", "Loud"); the meter exposes words, not numbers. */
    label: string;
    /** The slim variant of the baby screen. */
    compact?: boolean;
    testid?: string;
  };

  let { level, threshold = null, label, compact = false, testid }: Props = $props();

  // Bars and tick share the slider's decibel scale (`meterFraction`).
  const bars = $derived(meterBars(meterFraction(level)));
</script>

<div
  class="gs-meter"
  data-compact={compact || undefined}
  data-testid={testid}
  role="meter"
  aria-valuemin="0"
  aria-valuemax="1"
  aria-valuenow={Math.min(1, Math.max(0, level))}
  aria-valuetext={label}
  aria-label={label}
  style:--gs-threshold={threshold === null ? undefined : `${meterFraction(threshold) * 100}%`}
>
  {#each bars as bar, i (i)}
    <span class="gs-meter__bar" data-on={bar.on || undefined} data-zone={bar.zone}></span>
  {/each}
  {#if threshold !== null}<span class="gs-meter__threshold"></span>{/if}
</div>
