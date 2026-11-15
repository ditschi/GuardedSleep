<script lang="ts">
  import { useController } from "../contract";
  import { t } from "../i18n/store";
  import Button from "./Button.svelte";
  import { formatDurationWords } from "./format";
  import Glyph from "./Glyph.svelte";
  import { missedResultOf } from "./parent-view";

  /**
   * "Connected again" (frame 14): the duration in words and the missed-sound result, until the
   * parent taps OK. It never dismisses itself; a new outage replaces it (the model clears the state).
   */
  type Props = { restoredAfterMs: number };

  let { restoredAfterMs }: Props = $props();

  const controller = useController();
  const { parent } = controller;

  const missed = $derived($parent.missed);
  const result = $derived(missedResultOf(missed));
</script>

<section class="gs-card gs-restored" data-testid="restored_card">
  <h2 class="gs-restored__title" data-testid="restored_title">
    <Glyph name="check" />
    {$t("native.ui.linkAlarm.restored.title")}
  </h2>
  <p data-testid="restored_body">
    {$t("native.ui.connAlert.restored.body", {
      duration: formatDurationWords($t, restoredAfterMs),
    })}
  </p>
  <p class="gs-hint" data-testid="restored_result" data-result={result}>
    {#if result === "none"}
      {$t("native.ui.connAlert.restored.none")}
    {:else if result === "some"}
      {$t("native.ui.connAlert.restored.some", { count: missed?.count ?? 0 })}
    {:else}
      {$t("native.ui.connAlert.restored.unknown")}
    {/if}
  </p>
  <Button block testid="restored_ok" onclick={() => controller.dismissRestored()}>
    {$t("native.ui.common.ok")}
  </Button>
</section>
