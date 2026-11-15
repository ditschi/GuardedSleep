<script lang="ts">
  import type { ListenMode } from "../contract";
  import { t } from "../i18n/store";
  import Glyph from "./Glyph.svelte";

  /**
   * Listen is a tap toggle (`redesign-calm-ui` "Listen is a tap toggle"): one tap starts it, one
   * tap stops it, the whole bar fills while it is on. No hold, slide or lock gesture. The 60 minute
   * limit is the platform model's, not this button's.
   */
  type Props = {
    mode: ListenMode;
    /** Listening needs a connected link. */
    enabled: boolean;
    onchange: (mode: ListenMode) => void;
    testid?: string;
    /** `alert` is the compact row on the noise alert screen. */
    variant?: "primary" | "alert";
  };

  let { mode, enabled, onchange, testid, variant = "primary" }: Props = $props();

  const on = $derived(enabled && mode === "on");
  const stateId = $props.id();

  const title = $derived(
    !enabled
      ? $t("native.ui.listen.disabled")
      : on
        ? $t("native.ui.listen.on")
        : $t("native.ui.listen.idle"),
  );
  const sub = $derived(
    !enabled
      ? $t("native.ui.listen.disabledSub")
      : on
        ? $t("native.ui.listen.onSub")
        : $t("native.ui.listen.idleSub"),
  );
</script>

<button
  type="button"
  class="gs-listen"
  data-variant={variant}
  data-listen={on ? "on" : "off"}
  data-testid={testid}
  disabled={!enabled}
  aria-pressed={on}
  aria-describedby={stateId}
  onclick={() => onchange(on ? "off" : "on")}
>
  <span class="gs-listen__icon"><Glyph name={on ? "stop" : "headphones"} /></span>
  <span class="gs-listen__text">
    <span class="gs-listen__title">{title}</span>
    <span class="gs-listen__sub">{sub}</span>
  </span>
</button>
<!-- The state in words for assistive technology, besides aria-pressed. -->
<span id={stateId} class="gs-sr-only">
  {$t(on ? "native.ui.listen.stateOn" : "native.ui.listen.stateOff")}
</span>
