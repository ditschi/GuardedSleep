<script lang="ts">
  import { onMount } from "svelte";
  import { useController } from "../lib/contract";
  import type { MessageKey } from "../lib/i18n/keys";
  import { t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import AmbientBar from "../lib/ui/AmbientBar.svelte";
  import Button from "../lib/ui/Button.svelte";
  import { formatDuration } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import { coreMessageKey } from "../lib/ui/messages";
  import { downForMs, type FullScreenKind } from "../lib/ui/parent-view";
  import { everySecond } from "../lib/ui/ticker";

  /**
   * S11: the link alarm and its sibling full screens (the baby device stopped, its battery is
   * almost empty, the devices are no longer paired). "I'm checking" only exists for a lost
   * connection; the others close with OK.
   */
  type Props = {
    kind: Exclude<FullScreenKind, "noise">;
    /** OK: closes the screen for the current situation. */
    onok: () => void;
  };

  let { kind, onok }: Props = $props();

  const controller = useController();
  const router = useRouter();
  const { app, parent } = controller;

  let heading: HTMLElement | undefined = $state();
  onMount(() => heading?.focus());

  const name = $derived($app.peerName ?? $t("native.ui.common.babyUnitDefault"));
  const urgent = $derived($parent.link.level >= 3);

  const title = $derived.by(() => {
    switch (kind) {
      case "link_lost":
        return $t(
          urgent ? "native.ui.linkAlarm.lost.titleUrgent" : "native.ui.linkAlarm.lost.title",
          { name },
        );
      case "baby_stopped":
        return $t("native.ui.linkAlarm.stopped.title", { name });
      case "battery_critical":
        return $t("native.ui.linkAlarm.battery.title");
      case "needs_pairing":
        return $t("native.ui.linkAlarm.needsPairing.title");
    }
  });

  const cause = $derived.by((): MessageKey | null => {
    const hint = $parent.link.causeHint;
    return hint === null ? null : coreMessageKey(hint);
  });

  const steps: MessageKey[] = [
    "native.ui.help.reconnect.step1",
    "native.ui.help.reconnect.step2",
    "native.ui.help.reconnect.step3",
  ];

  function stopMonitoring(): void {
    controller.stopMonitoring();
    router.navigate("/");
  }

  function pairAgain(): void {
    controller.forgetPairing();
    router.navigate("/parent/pair");
  }
</script>

<div
  class="gs-fullscreen"
  data-kind="link"
  data-variant={kind}
  data-testid="parent_link_alarm"
  role="alertdialog"
  aria-modal="true"
  aria-labelledby="link-alarm-title"
>
  <div class="gs-fullscreen__inner">
    <AmbientBar testid="link_alarm_bar" />

    <div class="gs-stack gs-fullscreen__top">
      <h1 id="link-alarm-title" tabindex="-1" bind:this={heading} data-testid="link_alarm_title">
        {title}
      </h1>

      <div class="gs-fullscreen__body" data-testid="link_alarm_body">
        {#if kind === "link_lost"}
          <p data-testid="link_alarm_duration">
            {$t("native.ui.link.lostFor", {
              duration: formatDuration($t, downForMs($parent.link, $everySecond.elapsedMs)),
            })}
          </p>
          <p>{$t("native.ui.linkAlarm.lost.body")}</p>
          {#if cause}<p data-testid="link_alarm_cause">{$t(cause)}</p>{/if}
        {:else if kind === "baby_stopped"}
          <p>{$t("native.ui.linkAlarm.stopped.body")}</p>
        {:else if kind === "battery_critical"}
          <p>{$t("native.ui.linkAlarm.battery.body")}</p>
        {:else}
          <p>{$t(coreMessageKey($parent.link.failureKey) ?? "core.pairingKeyMismatch")}</p>
        {/if}
      </div>
    </div>

    {#if kind === "link_lost" || kind === "baby_stopped"}
      <details data-testid="link_alarm_help">
        <summary>{$t("native.ui.help.reconnect.open")}</summary>
        <ul>
          {#each steps as step (step)}
            <li>{$t(step)}</li>
          {/each}
        </ul>
      </details>
    {/if}

    <div class="gs-fill"></div>

    {#if kind === "link_lost"}
      <Button
        variant="alert"
        size="large"
        block
        testid="link_alarm_ack"
        onclick={() => controller.acknowledgeAlarm()}
      >
        {$t("native.ui.linkAlarm.ack")}
      </Button>
      <p class="gs-hint" data-testid="link_alarm_ack_note">
        {$t("native.ui.linkAlarm.ackNote", { time: formatDuration($t, 5 * 60_000) })}
      </p>
    {:else if kind === "needs_pairing"}
      <Button variant="alert" size="large" block testid="link_alarm_pair_again" onclick={pairAgain}>
        <Glyph name="refresh" />
        {$t("native.ui.link.pairAgain")}
      </Button>
    {:else}
      <Button variant="alert" size="large" block testid="link_alarm_ok" onclick={onok}>
        {$t("native.ui.common.ok")}
      </Button>
    {/if}
    <Button variant="alert-outline" block testid="link_alarm_stop" onclick={stopMonitoring}>
      {$t("native.ui.parent.stop")}
    </Button>
  </div>
</div>
