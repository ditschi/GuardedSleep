<script lang="ts">
  import Glyph from "./Glyph.svelte";
  import { TONE_GLYPH, type GlyphName, type Tone } from "./glyphs";

  type Props = {
    tone?: Tone;
    /** The state in words; colour and icon repeat it, they never replace it. */
    label: string;
    icon?: GlyphName;
    /** Assertive for escalation level 2 and above (alpha-ux-flows.md section 6.1). */
    assertive?: boolean;
    testid?: string;
  };

  let { tone = "neutral", label, icon, assertive = false, testid }: Props = $props();
</script>

<div
  class="gs-status-bar"
  data-tone={tone}
  data-testid={testid}
  role={assertive ? "alert" : "status"}
>
  <Glyph name={icon ?? TONE_GLYPH[tone]} />
  <span>{label}</span>
</div>
