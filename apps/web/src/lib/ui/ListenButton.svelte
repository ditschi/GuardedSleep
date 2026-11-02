<script lang="ts">
  import type { ListenMode } from "../contract";
  import { t } from "../i18n/store";
  import Glyph from "./Glyph.svelte";

  type Props = {
    mode: ListenMode;
    /** Listening needs a connected link. */
    enabled: boolean;
    onchange: (mode: ListenMode) => void;
    testid?: string;
    /** `large` is the 72 dp primary action of the monitoring screen. */
    variant?: "primary" | "alert";
  };

  let { mode, enabled, onchange, testid, variant = "primary" }: Props = $props();

  /** Upward drag in pixels that turns "hold" into "keep listening" (alpha-ux-flows.md 3.8). */
  const LOCK_DISTANCE = 56;

  let startY = 0;
  let pointerId: number | null = null;
  // Set when a press only ended a locked session, so its release does not start anything.
  let endedLock = false;

  function onpointerdown(event: PointerEvent): void {
    if (!enabled || event.button !== 0) return;
    // Keeps the release and the upward drag coming even when the finger leaves the button.
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    pointerId = event.pointerId;
    startY = event.clientY;
    if (mode === "locked") {
      endedLock = true;
      onchange("off");
      return;
    }
    endedLock = false;
    onchange("hold");
  }

  function onpointermove(event: PointerEvent): void {
    if (pointerId !== event.pointerId || mode !== "hold") return;
    if (startY - event.clientY >= LOCK_DISTANCE) onchange("locked");
  }

  function onpointerup(event: PointerEvent): void {
    if (pointerId !== event.pointerId) return;
    pointerId = null;
    // A release ends a plain hold; a locked session goes on until the next tap.
    if (!endedLock && mode === "hold") onchange("off");
  }

  // Keyboard and assistive technology activate with `click` without a pointer; they switch
  // between off and locked, because holding a key down is not something every user can do.
  function onclick(event: MouseEvent): void {
    if (!enabled || event.detail !== 0) return;
    onchange(mode === "off" ? "locked" : "off");
  }

  const label = $derived(
    !enabled
      ? $t("native.ui.listen.disabled")
      : mode === "locked"
        ? $t("native.ui.listen.locked")
        : mode === "hold"
          ? $t("native.ui.listen.holding")
          : $t("native.ui.listen.idle"),
  );
</script>

<button
  type="button"
  class="gs-listen"
  data-variant={variant}
  data-listen={mode}
  data-testid={testid}
  disabled={!enabled}
  aria-pressed={mode !== "off"}
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  {onclick}
>
  <Glyph name="headphones" />
  <span>{label}</span>
</button>
{#if enabled && mode === "hold"}
  <p class="gs-hint" role="status">{$t("native.ui.listen.lockHint")}</p>
{/if}
