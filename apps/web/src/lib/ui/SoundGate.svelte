<script lang="ts">
  import { t } from "../i18n/store";
  import Button from "./Button.svelte";
  import Glyph from "./Glyph.svelte";

  /**
   * "Tap to turn on alert sound" (`web-client` "Same monitor in the browser", frame 36). Browsers
   * keep audio locked until a user gesture, so the browser parent shows this panel in place of the
   * monitor's hero area; the status bar above it and the dock below stay visible. The monitor owns
   * the placement and also unlocks on the first tap of any other control.
   */
  type Props = {
    onunlock: () => void;
    testid?: string;
  };

  let { onunlock, testid = "sound_gate" }: Props = $props();
</script>

<section class="gs-sound-gate" data-testid={testid} aria-labelledby="{testid}_title">
  <Glyph name="sound" />
  <h2 id="{testid}_title" class="gs-sound-gate__title">
    {$t("webScreens.parent.soundGateTitle")}
  </h2>
  <p class="gs-muted">{$t("webScreens.parent.soundGateBody")}</p>
  <Button variant="primary" size="large" block testid="{testid}_unlock" onclick={onunlock}>
    {$t("webScreens.parent.soundGate")}
  </Button>
</section>
