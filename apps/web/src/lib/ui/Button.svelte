<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    variant?: "primary" | "tonal" | "outline" | "text" | "danger" | "alert" | "alert-outline";
    /** `large` is the 72 dp primary action (Listen, Allow camera). */
    size?: "normal" | "large";
    block?: boolean;
    disabled?: boolean;
    type?: "button" | "submit";
    /** Stable test id (`<screen>_<element>`, alpha-ux-flows.md section 1.2). */
    testid?: string;
    /** Toggle buttons: sets `aria-pressed`. */
    pressed?: boolean;
    /** Accessible name when the visible content is an icon only. */
    label?: string;
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  };

  let {
    variant = "primary",
    size = "normal",
    block = false,
    disabled = false,
    type = "button",
    testid,
    pressed,
    label,
    onclick,
    children,
  }: Props = $props();
</script>

<button
  class="gs-btn"
  data-variant={variant}
  data-size={size}
  data-block={block || undefined}
  data-testid={testid}
  {type}
  {disabled}
  aria-pressed={pressed}
  aria-label={label}
  {onclick}
>
  {@render children()}
</button>
