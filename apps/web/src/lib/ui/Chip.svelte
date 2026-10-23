<script lang="ts">
  import Glyph from "./Glyph.svelte";
  import type { GlyphName, Tone } from "./glyphs";

  type Props = {
    tone?: Tone;
    label: string;
    icon?: GlyphName;
    testid?: string;
  };

  let { tone = "neutral", label, icon, testid }: Props = $props();
</script>

<span class="gs-chip" data-tone={tone} data-testid={testid}>
  {#if icon}<Glyph name={icon} />{/if}
  <span>{label}</span>
</span>
