<script lang="ts">
  import { useController } from "../lib/contract";
  import { t } from "../lib/i18n/store";
  import AlertTemplate from "../lib/ui/AlertTemplate.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Glyph from "../lib/ui/Glyph.svelte";
  import { connectionName, statusHeadline, statusKindOf } from "../lib/ui/parent-view";

  /**
   * The test variant of the noise alert: the same template and palette as a real alert, with the
   * words of a confirmation (`parent-alerts` "Test alert"). The baby device stayed silent; OK ends
   * the test and the monitor then asks "Did you hear and feel it?".
   */
  let { onclose }: { onclose: () => void } = $props();

  const controller = useController();
  const { app, parent, settings } = controller;

  const stripText = $derived(
    statusHeadline($t, statusKindOf($parent.link), $parent.link, connectionName($app.peerName)),
  );

  function ok(): void {
    controller.stopAlertSound();
    onclose();
  }
</script>

<AlertTemplate
  palette="noise"
  label={$t("native.ui.alert.testBadge")}
  headline={$t("native.ui.noiseAlert.testHeadline")}
  headlineId="test-alert-title"
  headlineTestid="test_alert_title"
  testid="parent_test_alert"
  sentence={$t(
    $settings.alertStyle === "discreet"
      ? "native.ui.probe.styleDiscreet"
      : "native.ui.probe.styleSound",
  )}
  sentenceTestid="test_alert_style"
>
  {#snippet strip()}
    <Glyph name="alert" />
    <span data-testid="alert_connection_bar">{stripText}</span>
  {/snippet}
  {#snippet body()}
    <p data-testid="test_alert_silent">{$t("native.ui.noiseAlert.testSilent")}</p>
  {/snippet}
  {#snippet action()}
    <Button block testid="test_alert_ok" onclick={ok}>{$t("native.ui.common.ok")}</Button>
  {/snippet}
</AlertTemplate>
