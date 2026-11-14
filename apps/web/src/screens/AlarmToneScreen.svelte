<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    ALARM_TONE_IDS,
    HARD_OF_HEARING_TONE,
    useController,
    type AlarmToneId,
  } from "../lib/contract";
  import { ALARM_TONE_COPY, storeAlarmToneSetting } from "../lib/alarm-tone";
  import { t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import Button from "../lib/ui/Button.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";

  const controller = useController();
  const router = useRouter();
  const { app, settings, alarmTonePlayback } = controller;

  // Parent only: the other roles (and a reload, which forgets the role) go back to the settings.
  $effect(() => {
    if ($app.role !== "parent") router.navigate("/settings", { replace: true });
  });

  // Leaving the screen stops any preview or test.
  onDestroy(() => controller.stopAlarmTone());

  function choose(id: AlarmToneId): void {
    controller.updateSettings((s) => ({ ...s, connectionAlarmTone: id }));
    storeAlarmToneSetting(localStorage, id);
  }

  const previewing = (id: string): boolean =>
    $alarmTonePlayback.kind === "preview" && $alarmTonePlayback.id === id;
  const testing = $derived($alarmTonePlayback.kind === "test");

  function back(): void {
    router.navigate("/settings");
  }
</script>

<div class="gs-screen" data-screen="alarm_tone" data-testid="alarm_tone_picker">
  <ScreenHeader onback={back} backTestid="alarm_tone_back" />

  <h1 tabindex="-1" data-route-heading data-testid="alarm_tone_title">
    {$t("native.ui.settings.alarmTone.label")}
  </h1>

  <p class="gs-muted" data-testid="alarm_tone_explanation">
    {$t("native.ui.settings.alarmTone.explanation")}
  </p>

  <fieldset class="gs-group">
    <legend>{$t("native.ui.settings.alarmTone.label")}</legend>
    {#each ALARM_TONE_IDS as id (id)}
      <div class="gs-tone-row">
        <label class="gs-choice">
          <input
            type="radio"
            name="alarm-tone"
            value={id}
            checked={$settings.connectionAlarmTone === id}
            data-testid={`alarm_tone_${id}`}
            onchange={() => choose(id)}
          />
          <span>
            {$t(ALARM_TONE_COPY[id].name)}
            {#if id === HARD_OF_HEARING_TONE}
              <span class="gs-tone-tag" data-testid={`alarm_tone_tag_${id}`}>
                {$t("native.ui.settings.alarmTone.tagHardOfHearing")}
              </span>
            {/if}
            <span class="gs-hint">{$t(ALARM_TONE_COPY[id].desc)}</span>
          </span>
        </label>
        <Button
          variant="outline"
          testid={`alarm_tone_preview_${id}`}
          label={previewing(id)
            ? $t("native.ui.settings.alarmTone.stop")
            : $t("native.ui.settings.alarmTone.playNamed", { name: $t(ALARM_TONE_COPY[id].name) })}
          pressed={previewing(id)}
          onclick={() =>
            previewing(id) ? controller.stopAlarmTone() : controller.previewAlarmTone(id)}
        >
          {previewing(id)
            ? $t("native.ui.settings.alarmTone.stop")
            : $t("native.ui.settings.alarmTone.play")}
        </Button>
      </div>
      {#if id === HARD_OF_HEARING_TONE}
        <p class="gs-hint" data-testid="alarm_tone_hint">
          {$t("native.ui.settings.alarmTone.hintHardOfHearing")}
        </p>
      {/if}
    {/each}
  </fieldset>

  <div class="gs-group">
    <p class="gs-hint" data-testid="alarm_tone_test_warning">
      {$t("native.ui.settings.alarmTone.testWarning")}
    </p>
    {#if testing}
      <Button
        variant="outline"
        block
        testid="alarm_tone_test_stop"
        onclick={() => controller.stopAlarmTone()}
      >
        {$t("native.ui.settings.alarmTone.stop")}
      </Button>
    {:else}
      <Button
        variant="outline"
        block
        testid="alarm_tone_test"
        onclick={() => controller.testAlarmTone($settings.connectionAlarmTone)}
      >
        {$t("native.ui.settings.alarmTone.test")}
      </Button>
    {/if}
  </div>
</div>
