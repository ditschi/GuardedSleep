<script lang="ts">
  import { useController } from "../lib/contract";
  import { locale, t } from "../lib/i18n/store";
  import AlertTemplate from "../lib/ui/AlertTemplate.svelte";
  import Button from "../lib/ui/Button.svelte";
  import { formatClock, formatElapsed, levelWordKey, spokenDuration } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import ListenButton from "../lib/ui/ListenButton.svelte";
  import Meter from "../lib/ui/Meter.svelte";
  import { connectionName, statusHeadline, statusKindOf } from "../lib/ui/parent-view";
  import { everySecond } from "../lib/ui/ticker";

  /**
   * S10: a noise alert in the page on the shared template with the wine palette (frames 08 and 31),
   * over everything else but a lost connection. Listen comes first in the quiet row, then the two
   * pauses. No Undo here: a pause started from a running alert is final (the snackbar belongs to the
   * monitor).
   */
  let { onclose }: { onclose: () => void } = $props();

  const controller = useController();
  const { app, parent } = controller;

  const ended = $derived(!$parent.noiseAlertActive);
  const since = $derived($parent.noiseSinceWallMs);
  const sinceClock = $derived(since === null ? "" : formatClock(since, $locale));
  const ageMs = $derived(since === null ? 0 : Math.max(0, $everySecond.wallMs - since));
  const counter = $derived({
    text: formatElapsed(ageMs),
    label: $t("native.ui.noiseAlert.counterLabel", { time: sinceClock }),
    spoken: $t("native.ui.noiseAlert.counterA11y", {
      duration: spokenDuration($t, ageMs),
      time: sinceClock,
    }),
  });
  const stripText = $derived(
    statusHeadline($t, statusKindOf($parent.link), $parent.link, connectionName($app.peerName)),
  );

  function stopSound(): void {
    controller.stopAlertSound();
    onclose();
  }

  function pause(minutes: number): void {
    controller.snooze(minutes);
    controller.stopAlertSound();
    onclose();
  }
</script>

<AlertTemplate
  palette="noise"
  label={$t(ended ? "native.ui.noiseAlert.labelStopped" : "native.ui.noiseAlert.labelSound")}
  headline={$t("native.ui.alert.title")}
  headlineId="alert-title"
  headlineTestid="alert_title"
  testid="parent_noise_alert"
  counter={since === null ? undefined : counter}
  counterTestid="alert_since"
  sentence={$t(ended ? "native.ui.alert.ended" : "native.ui.noiseAlert.sentenceSound")}
  sentenceTestid="alert_sentence"
>
  {#snippet strip()}
    <Glyph name="alert" />
    <span data-testid="alert_connection_bar">{stripText}</span>
  {/snippet}
  {#snippet body()}
    {#if $parent.noiseIsTest}
      <span class="gs-alert__badge" data-testid="alert_test_badge">
        {$t("native.ui.alert.testBadge")}
      </span>
    {/if}
    <div class="gs-alert__panel">
      <Meter
        level={$parent.level}
        threshold={$parent.babyThreshold}
        label={$t("native.ui.parent.meterA11y", { level: $t(levelWordKey($parent.level)) })}
        testid="alert_meter"
      />
    </div>
  {/snippet}
  {#snippet quiet()}
    <div class="gs-alert__row">
      <ListenButton
        variant="alert"
        mode={$parent.listen}
        enabled={$parent.link.phase === "connected"}
        onchange={(mode) => controller.setListen(mode)}
        testid="alert_listen"
      />
      <Button variant="alert-outline" testid="alert_snooze_10" onclick={() => pause(10)}>
        {$t("native.ui.snooze.pauseMinutes", { minutes: 10 })}
      </Button>
      <Button variant="alert-outline" testid="alert_snooze_30" onclick={() => pause(30)}>
        {$t("native.ui.snooze.pauseMinutes", { minutes: 30 })}
      </Button>
    </div>
  {/snippet}
  {#snippet action()}
    <Button block testid="alert_stop_sound" onclick={stopSound}>
      <span>{$t("native.ui.alert.stopSound")}</span>
      <span class="gs-btn__sub">{$t("native.ui.noiseAlert.stopSoundSub")}</span>
    </Button>
  {/snippet}
</AlertTemplate>
