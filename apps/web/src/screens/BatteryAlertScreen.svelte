<script lang="ts">
  import { useController } from "../lib/contract";
  import { t } from "../lib/i18n/store";
  import AlertTemplate from "../lib/ui/AlertTemplate.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Glyph from "../lib/ui/Glyph.svelte";
  import { connectionName, statusHeadline, statusKindOf } from "../lib/ui/parent-view";

  /**
   * The baby device's battery is almost empty (below 5 %, not charging) on the shared alert
   * template with the amber palette, so red stays reserved for "you can't hear the child". It is not
   * snoozable: the one action closes the screen for as long as the situation lasts. Low battery
   * (below 15 %) and unplugged are banners on the monitor, not this screen.
   */
  type Props = {
    /** OK, I'll plug it in: closes the screen for the current situation. */
    onok: () => void;
  };

  let { onok }: Props = $props();

  const { app, parent } = useController();

  const percent = $derived($parent.peer.batteryPct ?? 0);
  const stripText = $derived(
    statusHeadline($t, statusKindOf($parent.link), $parent.link, connectionName($app.peerName)),
  );
</script>

<AlertTemplate
  palette="battery"
  label={$t("native.ui.batteryAlert.label")}
  headline={$t("native.ui.batteryAlert.headline")}
  headlineId="battery-alert-title"
  headlineTestid="battery_alert_title"
  testid="battery_alert_template"
  counter={{
    text: $t("native.ui.monitor.batteryShort", { percent }),
    label: $t("native.ui.batteryAlert.counterLabel"),
    spoken: $t("native.ui.batteryAlert.counterA11y", { percent }),
  }}
  counterTestid="battery_alert_counter"
  sentence={$t("native.ui.batteryAlert.sentence")}
  sentenceTestid="battery_alert_sentence"
>
  {#snippet strip()}
    <Glyph name="battery" />
    <span>{stripText}</span>
  {/snippet}
  {#snippet body()}
    <p class="gs-alert__sentence" data-testid="battery_alert_silent_note">
      {$t("native.ui.batteryAlert.silentNote")}
    </p>
  {/snippet}
  {#snippet action()}
    <Button block testid="battery_alert_ok" onclick={onok}>
      <span>{$t("native.ui.batteryAlert.action")}</span>
      <span class="gs-btn__sub">{$t("native.ui.batteryAlert.actionSub")}</span>
    </Button>
  {/snippet}
</AlertTemplate>
