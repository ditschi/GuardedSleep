<script lang="ts">
  import Button from "./Button.svelte";
  import Glyph from "./Glyph.svelte";
  import { TONE_GLYPH, type Tone } from "./glyphs";

  type Props = {
    tone?: Tone;
    /** The message, already translated. */
    message: string;
    /** Set (the translated "Dismiss") to show a dismiss button together with `onDismiss`. */
    dismissLabel?: string;
    onDismiss?: () => void;
    testid?: string;
  };

  let { tone = "neutral", message, dismissLabel, onDismiss, testid }: Props = $props();
</script>

<div
  class="gs-banner"
  data-tone={tone}
  data-testid={testid}
  role={tone === "bad" ? "alert" : "status"}
>
  <Glyph name={tone === "neutral" ? "info" : TONE_GLYPH[tone]} />
  <p class="gs-banner__body">{message}</p>
  {#if dismissLabel && onDismiss}
    <Button variant="text" onclick={onDismiss}>{dismissLabel}</Button>
  {/if}
</div>
