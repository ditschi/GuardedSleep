<script lang="ts">
  import { onMount } from "svelte";
  import { useController } from "../lib/contract";
  import { locale, t } from "../lib/i18n/store";
  import AmbientBar from "../lib/ui/AmbientBar.svelte";
  import Button from "../lib/ui/Button.svelte";
  import { formatClock, levelWordKey } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import ListenButton from "../lib/ui/ListenButton.svelte";
  import Meter from "../lib/ui/Meter.svelte";

  /** S10: a noise alert in the page, over everything else but a lost connection. */
  let { onclose }: { onclose: () => void } = $props();

  const controller = useController();
  const { parent, settings } = controller;
  const discreet = $derived($settings.alertStyle === "discreet");

  let heading: HTMLElement | undefined = $state();
  // The alert takes the focus so a screen reader announces it and Enter reaches "Stop sound".
  onMount(() => heading?.focus());

  const since = $derived(
    $parent.noiseSinceWallMs === null ? null : formatClock($parent.noiseSinceWallMs, $locale),
  );

  function stopSound(): void {
    controller.stopAlertSound();
    onclose();
  }

  function snooze(minutes: number): void {
    controller.snooze(minutes);
    controller.stopAlertSound();
    onclose();
  }
</script>

<div
  class="gs-fullscreen"
  data-kind="noise"
  data-testid="parent_noise_alert"
  role="alertdialog"
  aria-modal="true"
  aria-labelledby="alert-title"
>
  <div class="gs-fullscreen__inner">
    <AmbientBar testid="alert_connection_bar" />

    <div class="gs-fullscreen__hero">
      {#if $parent.noiseIsTest}
        <span class="gs-fullscreen__badge" data-testid="alert_test_badge">
          {$t("native.ui.alert.testBadge")}
        </span>
      {/if}
      <span class="gs-fullscreen__icon"><Glyph name="alert" /></span>
      <h1 id="alert-title" tabindex="-1" bind:this={heading} data-testid="alert_title">
        {$t("native.ui.alert.title")}
      </h1>
      {#if since}
        <p data-testid="alert_since">{$t("native.ui.alert.since", { time: since })}</p>
      {/if}
    </div>

    <div class="gs-fullscreen__panel">
      <Meter
        level={$parent.level}
        threshold={$parent.babyThreshold}
        label={$t("native.ui.parent.meterA11y", { level: $t(levelWordKey($parent.level)) })}
        testid="alert_meter"
      />
    </div>

    {#if discreet}
      <p class="gs-hint" data-testid="alert_discreet_hint">{$t("native.ui.alert.discreetHint")}</p>
    {/if}

    <div class="gs-fill"></div>

    <!-- The web client has no vibration, so discreet only offers to dismiss. -->
    <Button variant="alert" size="large" block testid="alert_stop_sound" onclick={stopSound}>
      <Glyph name="stop" />
      {$t(discreet ? "native.ui.alert.dismiss" : "native.ui.alert.stopSound")}
    </Button>
    <div class="gs-action-row">
      <Button variant="alert-outline" testid="alert_snooze_10" onclick={() => snooze(10)}>
        {$t("native.ui.snooze.minutes", { minutes: 10 })}
      </Button>
      <Button variant="alert-outline" testid="alert_snooze_30" onclick={() => snooze(30)}>
        {$t("native.ui.snooze.minutes", { minutes: 30 })}
      </Button>
    </div>
    <ListenButton
      variant="alert"
      mode={$parent.listen}
      enabled={$parent.link.phase === "connected"}
      onchange={(mode) => controller.setListen(mode)}
      testid="alert_listen"
    />
  </div>
</div>
