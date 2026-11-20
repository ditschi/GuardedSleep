<script lang="ts">
  import { onMount } from "svelte";
  import { useController } from "../lib/contract";
  import { locale, t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import AlertTemplate from "../lib/ui/AlertTemplate.svelte";
  import Button from "../lib/ui/Button.svelte";
  import CauseCard from "../lib/ui/CauseCard.svelte";
  import { formatClock, formatElapsed, spokenDuration } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import { coreMessageKey } from "../lib/ui/messages";
  import {
    connectionName,
    statusHeadline,
    alertStageOf,
    downForMs,
    elapsedToWall,
    statusKindOf,
    type FullScreenKind,
  } from "../lib/ui/parent-view";
  import StatusBar from "../lib/ui/StatusBar.svelte";
  import StopDialog from "../lib/ui/StopDialog.svelte";
  import { everySecond } from "../lib/ui/ticker";

  /**
   * S11: the connection alert on the shared template (frames 12, 13, 25, 26) and its sibling full
   * screens (the baby device stopped, the devices are no longer paired). The stage (vibrating, loud, paused) and every time come from the core's level,
   * `next_level_at_ms` and `acknowledged_until`; this screen has no threshold of its own.
   */
  type Props = {
    kind: Exclude<FullScreenKind, "noise" | "test_alert" | "battery_critical">;
    /** OK: closes the screen for the current situation. */
    onok: () => void;
  };

  let { kind, onok }: Props = $props();

  const controller = useController();
  const router = useRouter();
  const { app, parent } = controller;

  let heading: HTMLElement | undefined = $state();
  onMount(() => heading?.focus());

  let stopOpen = $state(false);
  let wrongOpen = $state(false);

  const name = $derived(connectionName($app.peerName));
  const link = $derived($parent.link);
  const stage = $derived(alertStageOf(link, $everySecond.elapsedMs));
  const firstConnection = $derived(link.phase === "connecting");
  const clock = (atElapsedMs: number): string =>
    formatClock(elapsedToWall(atElapsedMs, $everySecond.elapsedMs, $everySecond.wallMs), $locale);

  const headline = $derived.by(() => {
    if (stage === "paused") {
      return $t("native.ui.connAlert.nextBack", {
        time: clock(link.acknowledgedUntilElapsedMs ?? $everySecond.elapsedMs),
      });
    }
    if (stage === "loud") return statusHeadline($t, "still_no_connection", link, name);
    return firstConnection
      ? statusHeadline($t, "couldnt_connect", link, name)
      : $t("native.ui.connAlert.headlineCheck");
  });

  const sentence = $derived.by(() => {
    if (stage === "loud") return $t("native.ui.connAlert.goNow");
    if (stage === "paused") return $t("native.ui.connAlert.pausedBody");
    const lead = $t(
      firstConnection ? "native.ui.connAlert.neverStarted" : "native.ui.connAlert.cantHear",
    );
    return link.nextLevelAtElapsedMs === null
      ? lead
      : `${lead} ${$t("native.ui.connAlert.nextLoud", { time: clock(link.nextLevelAtElapsedMs) })}`;
  });

  const down = $derived(downForMs(link, $everySecond.elapsedMs));
  const counter = $derived({
    text: formatElapsed(down),
    label: $t("native.ui.connAlert.counterLabel"),
    spoken: $t("native.ui.connAlert.counterA11y", { duration: spokenDuration($t, down) }),
  });
  const progress = $derived(
    stage === "loud"
      ? { active: 4, steps: 4, words: $t("native.ui.connAlert.progressLoud") }
      : stage === "paused"
        ? { active: 3, steps: 4, words: $t("native.ui.connAlert.progressPaused") }
        : { active: 3, steps: 4, words: $t("native.ui.connAlert.progressAlerting") },
  );
  const label = $derived(
    $t(
      stage === "loud"
        ? "native.ui.connAlert.labelLoud"
        : stage === "paused"
          ? "native.ui.connAlert.labelPaused"
          : "native.ui.connAlert.labelAlerting",
    ),
  );
  const stripText = $derived(statusHeadline($t, statusKindOf(link), link, name));

  // The other full screens keep their own title and text.
  const title = $derived.by(() => {
    switch (kind) {
      case "baby_stopped":
        return $t("native.ui.linkAlarm.stopped.title", {
          name: name ?? $t("native.ui.common.babyUnitDefault"),
        });
      case "needs_pairing":
        return $t("native.ui.linkAlarm.needsPairing.title");
      case "link_lost":
        return headline;
    }
  });

  function stopMonitoring(): void {
    stopOpen = false;
    controller.stopMonitoring();
    router.navigate("/");
  }

  function stopHereToo(): void {
    controller.acknowledgePeerStopped();
    router.navigate("/");
  }

  function chooseAnother(): void {
    wrongOpen = false;
    controller.chooseAnotherConnection();
    router.navigate("/");
  }

  function pairAgain(): void {
    controller.forgetPairing();
    router.navigate("/parent/pair");
  }
</script>

{#if kind === "link_lost"}
  <AlertTemplate
    palette={stage === "loud" ? "alarm" : "lost"}
    {label}
    {headline}
    headlineId="link-alarm-title"
    headlineTestid="link_alarm_title"
    testid="parent_link_alarm"
    {counter}
    counterTestid="link_alarm_since"
    {sentence}
    sentenceTestid="link_alarm_sentence"
    {progress}
  >
    {#snippet strip()}
      <Glyph name="linkOff" />
      <span>{stripText}</span>
    {/snippet}
    {#snippet body()}
      <CauseCard hint={link.causeHint} ontrynow={() => controller.tryReconnectNow()} />
      {#if firstConnection}
        <!-- The way back to the saved connections; from this screen (level 2 and up) it goes
             through the stop confirmation, so a running alarm is never left by one tap. -->
        <button
          type="button"
          class="gs-btn"
          data-variant="text"
          data-testid="link_alarm_wrong_connection"
          onclick={() => (wrongOpen = true)}
        >
          {$t("native.ui.connAlert.wrongConnection")}
        </button>
      {/if}
    {/snippet}
    {#snippet quiet()}
      <button
        type="button"
        class="gs-btn"
        data-variant="text"
        data-testid="link_alarm_stop"
        onclick={() => (stopOpen = true)}
      >
        {$t("native.ui.parent.stop")}
      </button>
    {/snippet}
    {#snippet action()}
      {#if stage !== "paused"}
        <Button block testid="link_alarm_ack" onclick={() => controller.acknowledgeAlarm()}>
          {$t("native.ui.linkAlarm.ack")}
        </Button>
      {/if}
    {/snippet}
  </AlertTemplate>
  <StopDialog bind:open={stopOpen} {name} onconfirm={stopMonitoring} />
  <StopDialog
    bind:open={wrongOpen}
    {name}
    onconfirm={chooseAnother}
    testid="dialog_choose_other"
    confirmTestid="dialog_choose_other_confirm"
    cancelTestid="dialog_choose_other_cancel"
  />
{:else}
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
      <StatusBar testid="link_alarm_bar" />

      <div class="gs-stack gs-fullscreen__top">
        <h1 id="link-alarm-title" tabindex="-1" bind:this={heading} data-testid="link_alarm_title">
          {title}
        </h1>

        <div class="gs-fullscreen__body" data-testid="link_alarm_body">
          {#if kind === "baby_stopped"}
            <p>{$t("native.ui.linkAlarm.stopped.body")}</p>
          {:else}
            <p>{$t(coreMessageKey($parent.link.failureKey) ?? "core.pairingKeyMismatch")}</p>
          {/if}
        </div>
      </div>

      <div class="gs-fill"></div>

      {#if kind === "needs_pairing"}
        <Button
          variant="alert"
          size="large"
          block
          testid="link_alarm_pair_again"
          onclick={pairAgain}
        >
          <Glyph name="refresh" />
          {$t("native.ui.link.pairAgain")}
        </Button>
      {:else if kind === "baby_stopped"}
        <!-- The baby device stopped on purpose: the primary answer ends monitoring here too,
             through the core (it ignores a stale tap). OK alone keeps monitoring. -->
        <Button
          variant="alert"
          size="large"
          block
          testid="link_alarm_stop_here"
          onclick={stopHereToo}
        >
          {$t("native.ui.linkAlarm.stopped.stopHere")}
        </Button>
        <Button variant="alert-outline" block testid="link_alarm_ok" onclick={onok}>
          {$t("native.ui.common.ok")}
        </Button>
      {:else}
        <Button variant="alert" size="large" block testid="link_alarm_ok" onclick={onok}>
          {$t("native.ui.common.ok")}
        </Button>
      {/if}
      {#if kind !== "baby_stopped"}
        <Button
          variant="alert-outline"
          block
          testid="link_alarm_stop"
          onclick={() => (stopOpen = true)}
        >
          {$t("native.ui.parent.stop")}
        </Button>
      {/if}
    </div>
  </div>
  <StopDialog bind:open={stopOpen} {name} onconfirm={stopMonitoring} />
{/if}
