<script lang="ts">
  import { qrShape } from "./qr";

  type Props = {
    /** The invite text. It is only drawn, never written into the DOM as text. */
    text: string;
    /** Accessible name; the code itself says nothing to a screen reader. */
    label: string;
    /** "Make code brighter": full-strength modules instead of the default ≤ 50 % brightness. */
    bright?: boolean;
    testid?: string;
  };

  let { text, label, bright = false, testid }: Props = $props();

  const shape = $derived(qrShape(text));
</script>

<div
  class="gs-qr"
  role="img"
  aria-label={label}
  data-testid={testid}
  data-bright={bright || undefined}
>
  <svg viewBox={`0 0 ${shape.size} ${shape.size}`} aria-hidden="true" focusable="false">
    <path d={shape.path} />
  </svg>
</div>
