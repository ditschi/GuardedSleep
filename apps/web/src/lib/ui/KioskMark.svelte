<script lang="ts">
  import type { KioskMarkShape } from "./kiosk";

  /**
   * The baby device's link mark along the edge of the page (`connection-status` "Baby unit edge
   * indicator"), told by shape: a short bar, a dashed top edge, a slow-pulsing frame, and a small
   * square next to the bar for "needs attention". Colours are the kiosk mark colours, none brighter
   * than the cap; the pulse is static with reduced motion (CSS). Decorative: the status line says it
   * in words.
   */
  type Props = { shape: KioskMarkShape; attention: boolean };

  let { shape, attention }: Props = $props();
</script>

<div class="gs-kiosk-edge" aria-hidden="true" data-testid="baby_kiosk_edge" data-shape={shape}>
  {#if shape === "bar" || attention}
    <div class="gs-kiosk-bar" data-attention={attention || undefined}>
      <span class="gs-kiosk-bar__line" data-testid="baby_kiosk_mark_bar"></span>
      {#if attention}
        <span class="gs-kiosk-bar__square" data-testid="baby_kiosk_mark_attention"></span>
      {/if}
    </div>
  {/if}
  {#if shape === "dashed"}
    <span class="gs-kiosk-dashed" data-testid="baby_kiosk_mark_dashed"></span>
  {/if}
  {#if shape === "frame"}
    <span class="gs-kiosk-frame" data-testid="baby_kiosk_mark_frame"></span>
  {/if}
</div>
