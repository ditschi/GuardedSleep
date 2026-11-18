<script lang="ts">
  import { onMount, type Snippet } from "svelte";

  /**
   * The shared full-screen alert template (redesign-calm-ui design section 6): a flat status strip
   * hidden from assistive technology, a caps label, the headline (focus lands here on entry), the
   * counter with its word label, one sentence, decorative stage segments with their words, a body
   * slot (the cause card), a quiet slot at the end of the content and one filled action pinned
   * below. The body scrolls; the action stays pinned at 200 % text. The colours are the fixed
   * `--safety-*` palettes, never the colour scheme.
   */
  type Props = {
    /**
     * Which fixed palette: no connection (red), the loud alarm (brighter red, hazard band), the
     * child's sound (wine) or the baby device's empty battery (amber, so red stays for "you can't
     * hear the child").
     */
    palette: "lost" | "alarm" | "noise" | "battery";
    label: string;
    headline: string;
    headlineId: string;
    headlineTestid: string;
    testid: string;
    /** The big figure: digits as shown, the spoken form, and the word label under it. */
    counter?: { text: string; label: string; spoken: string };
    counterTestid?: string;
    sentence?: string;
    sentenceTestid?: string;
    progress?: { active: number; steps: number; words: string };
    strip?: Snippet;
    body?: Snippet;
    quiet?: Snippet;
    action?: Snippet;
  };

  let {
    palette,
    label,
    headline,
    headlineId,
    headlineTestid,
    testid,
    counter,
    counterTestid = "alert_counter",
    sentence,
    sentenceTestid = "alert_sentence",
    progress,
    strip,
    body,
    quiet,
    action,
  }: Props = $props();

  let heading: HTMLElement | undefined = $state();
  // Focus moves to the headline on entry; on close the router or the opener restores it.
  onMount(() => heading?.focus());
</script>

<div
  class="gs-alert"
  data-palette={palette}
  data-testid={testid}
  role="alertdialog"
  aria-modal="true"
  aria-labelledby={headlineId}
>
  {#if palette === "alarm"}<div class="gs-alert__hazard" aria-hidden="true"></div>{/if}
  {#if strip}
    <div class="gs-alert__strip" aria-hidden="true" data-testid="alert_strip">
      {@render strip()}
    </div>
  {/if}
  <div class="gs-alert__scroll">
    <p class="gs-alert__label" aria-hidden="true" data-testid="alert_label">{label}</p>
    <!-- A stage change is announced once, assertively; the counter below never is. -->
    <h1
      id={headlineId}
      tabindex="-1"
      bind:this={heading}
      aria-live="assertive"
      data-testid={headlineTestid}
    >
      {headline}
    </h1>
    {#if counter}
      <!-- Never a live region: a ticking figure would be read out every second. -->
      <div class="gs-alert__counter" data-testid={counterTestid}>
        <span class="gs-alert__figure" aria-hidden="true">{counter.text}</span>
        <span class="gs-sr-only">{counter.spoken}</span>
        <span class="gs-alert__counter-label" aria-hidden="true">{counter.label}</span>
      </div>
    {/if}
    {#if sentence}<p class="gs-alert__sentence" data-testid={sentenceTestid}>{sentence}</p>{/if}
    {#if progress}
      <div class="gs-alert__progress" data-testid="alert_progress">
        <div class="gs-alert__segments" aria-hidden="true">
          {#each Array.from({ length: progress.steps }, (_, i) => i) as i (i)}
            <span data-on={i < progress.active || undefined}></span>
          {/each}
        </div>
        <p>{progress.words}</p>
      </div>
    {/if}
    {#if body}{@render body()}{/if}
    {#if quiet}<div class="gs-alert__quiet">{@render quiet()}</div>{/if}
  </div>
  {#if action}<div class="gs-alert__action">{@render action()}</div>{/if}
</div>
