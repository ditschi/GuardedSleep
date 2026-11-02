<script lang="ts">
  import { GLYPHS, type GlyphName } from "./glyphs";

  // Decorative: the text next to an icon carries the meaning, so it is hidden from screen readers.
  let { name }: { name: GlyphName } = $props();
</script>

<svg class="gs-glyph" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
  <path d={GLYPHS[name]} />
</svg>
