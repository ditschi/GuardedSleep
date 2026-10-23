<script lang="ts">
  import { untrack } from "svelte";
  import { useController } from "../lib/contract";
  import type { MessageKey } from "../lib/i18n/keys";
  import { locale, t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import AmbientBar from "../lib/ui/AmbientBar.svelte";
  import {
    askForNotifications,
    browserNotifications,
    flashTitle,
    notifyAway,
  } from "../lib/ui/attention";
  import Banner from "../lib/ui/Banner.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Chip from "../lib/ui/Chip.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
  import { formatClock, formatDuration, levelWordKey } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import KeepAwakeNotice from "../lib/ui/KeepAwakeNotice.svelte";
  import ListenButton from "../lib/ui/ListenButton.svelte";
  import Meter from "../lib/ui/Meter.svelte";
  import {
    canRequestSilence,
    downForMs,
    extraChips,
    fixedChips,
    fullScreenOf,
    headlineOf,
    pruneDismissed,
    type FullScreenKind,
  } from "../lib/ui/parent-view";
  import { everySecond } from "../lib/ui/ticker";
  import LinkAlarmScreen from "./LinkAlarmScreen.svelte";
  import NoiseAlertScreen from "./NoiseAlertScreen.svelte";

  const controller = useController();
  const router = useRouter();
  const { app, parent, soundUnlocked } = controller;

  // ---- full screens ----------------------------------------------------------------------

  let dismissed = $state<ReadonlySet<FullScreenKind>>(new Set());
  $effect(() => {
    const before = untrack(() => dismissed);
    const after = pruneDismissed($parent, before);
    if (after !== before) dismissed = after;
  });
  const fullScreen = $derived(fullScreenOf($parent, $everySecond.elapsedMs, dismissed));

  function dismiss(kind: FullScreenKind): void {
    dismissed = new Set(dismissed).add(kind);
  }

  // What catches the eye while a full screen is up and the page is not in front: the tab title
  // flashes and, when the user allowed it, a system notification appears. The alert sound itself
  // is the platform layer's job.
  const away = $derived.by((): { title: MessageKey; body: MessageKey } | null => {
    switch (fullScreen) {
      case "noise":
        return $parent.noiseIsTest
          ? { title: "native.notif.noiseTestTitle", body: "native.notif.noiseTestText" }
          : { title: "native.notif.noiseTitle", body: "native.notif.noiseText" };
      case "link_lost":
        return { title: "native.notif.linkLostTitle", body: "native.notif.linkLostText" };
      case "baby_stopped":
        return { title: "native.notif.peerStoppedTitle", body: "native.notif.peerStoppedText" };
      case "battery_critical":
        return {
          title: "native.notif.batteryCriticalTitle",
          body: "native.notif.batteryCriticalText",
        };
      case "needs_pairing":
        return { title: "native.ui.linkAlarm.needsPairing.title", body: "core.pairingKeyMismatch" };
      case null:
        return null;
    }
  });
  $effect(() => {
    if (!away) return;
    const title = $t(away.title);
    const body = $t(away.body, { percent: untrack(() => $parent.peer.batteryPct ?? 0) });
    const stopFlash = flashTitle(document, title);
    const closeNote = notifyAway(browserNotifications(window), document, {
      title,
      body,
      tag: "guardedsleep-alert",
    });
    return () => {
      stopFlash();
      closeNote();
    };
  });

  // ---- monitor ---------------------------------------------------------------------------

  const connected = $derived($parent.link.phase === "connected");
  const headline = $derived(headlineOf($parent, $everySecond.elapsedMs, $everySecond.wallMs));
  const headlineText = $derived.by(() => {
    const clock = (ms: number): string => formatClock(ms, $locale);
    switch (headline.kind) {
      case "all_quiet":
        return $t("native.ui.parent.allQuietSince", { time: clock(headline.sinceWallMs) });
      case "sound_since":
        return $t("native.ui.parent.soundSince", { time: clock(headline.sinceWallMs) });
      case "sound_at":
        return $t("native.ui.parent.soundAt", {
          time: clock(headline.atWallMs),
          duration: formatDuration($t, headline.durationMs),
        });
      case "sound_snoozed":
        return $t("native.ui.parent.soundSnoozed", { time: clock(headline.atWallMs) });
      case "last_heard":
        return $t("native.ui.parent.lastHeard", { time: clock(headline.atWallMs) });
      case "no_contact":
        return $t("native.ui.parent.noContactYet");
      case "monitoring":
        return $t("native.ui.parent.monitoringNow");
    }
  });

  const meterLevel = $derived(connected ? $parent.level : 0);
  const meterLabel = $derived(
    connected
      ? $t("native.ui.parent.meterA11y", { level: $t(levelWordKey(meterLevel)) })
      : $t("native.ui.listen.disabled"),
  );

  const fixed = $derived(fixedChips($parent));
  const extras = $derived(extraChips($parent, $everySecond.wallMs));
  const shownExtras = $derived(extras.slice(0, 2));
  const hiddenCount = $derived(extras.length - shownExtras.length);

  const snoozeActive = $derived(
    $parent.snoozedUntilWallMs !== null && $parent.snoozedUntilWallMs > $everySecond.wallMs,
  );

  const missedText = $derived.by(() => {
    const m = $parent.missed;
    if (m === null) return null;
    const end = m.endWallMs ?? $everySecond.wallMs;
    const args = {
      start: formatClock(end - m.downMs, $locale),
      end: formatClock(end, $locale),
    };
    if (m.count === null || !m.complete) return $t("native.ui.missed.unknown", args);
    if (m.count === 0) return $t("native.ui.missed.none");
    return $t("native.ui.missed.some", {
      ...args,
      count: m.count,
      level: $t(levelWordKey(m.loudest ?? 0)),
    });
  });

  function unlockSound(): void {
    controller.unlockSound();
    // The same tap is the moment to ask for notifications; nothing changes if the user declines.
    void askForNotifications(browserNotifications(window));
  }

  // ---- sheets ----------------------------------------------------------------------------

  let snoozeOpen = $state(false);
  let probeOpen = $state(false);
  let moreOpen = $state(false);
  let stopOpen = $state(false);
  let helpOpen = $state(false);
  let questionOpen = $state(false);
  let localOnly = $state(false);
  let awaitingTest = $state(false);

  function snooze(minutes: number): void {
    controller.snooze(minutes);
    snoozeOpen = false;
  }

  function runTest(): void {
    probeOpen = false;
    localOnly = !connected;
    controller.requestTestAlert();
    // Connected: the baby device answers with a test alert, whose full screen ends the test.
    // Not connected: only this device sounded, so the question comes at once.
    if (connected) awaitingTest = true;
    else questionOpen = true;
  }

  // The question "Did you hear and feel it?" follows the test alert screen.
  let sawTestScreen = false;
  $effect(() => {
    const onTestScreen = fullScreen === "noise" && $parent.noiseIsTest;
    if (onTestScreen) sawTestScreen = true;
    else if (sawTestScreen && awaitingTest) {
      sawTestScreen = false;
      awaitingTest = false;
      questionOpen = true;
    }
  });

  function answer(heard: boolean): void {
    questionOpen = false;
    controller.reportTestResult(heard);
  }

  function stop(): void {
    stopOpen = false;
    controller.stopMonitoring();
    router.navigate("/");
  }

  const steps: MessageKey[] = [
    "native.ui.help.reconnect.step1",
    "native.ui.help.reconnect.step2",
    "native.ui.help.reconnect.step3",
  ];
</script>

{#if fullScreen === "noise"}
  <NoiseAlertScreen onclose={() => dismiss("noise")} />
{:else if fullScreen !== null}
  <LinkAlarmScreen kind={fullScreen} onok={() => dismiss(fullScreen)} />
{:else}
  <div class="gs-screen gs-screen--monitor" data-screen="parent_monitor">
    <div class="gs-ambient">
      <AmbientBar onhelp={() => (helpOpen = true)} />
    </div>

    {#if $parent.securityWarning}
      <Card tone="variant" testid="parent_security_banner">
        <h2>{$t("native.ui.parent.securityTitle")}</h2>
        <p>{$t("native.ui.parent.securityBody")}</p>
      </Card>
    {/if}

    <KeepAwakeNotice />

    {#if $parent.restoredAfterMs !== null}
      <Banner
        tone="ok"
        message={$t("native.ui.link.restoredAfter", {
          duration: formatDuration($t, $parent.restoredAfterMs),
        })}
        testid="parent_restored_toast"
      />
    {/if}

    {#if missedText}
      <Card tone="variant" testid="card_missed_events">
        <p>{missedText}</p>
        <Button variant="text" onclick={() => controller.dismissMissedEvents()}>
          {$t("webShell.dismiss")}
        </Button>
      </Card>
    {/if}

    {#if !$soundUnlocked}
      <Button variant="tonal" testid="parent_sound_gate" onclick={unlockSound}>
        <Glyph name="sound" />
        {$t("webScreens.parent.soundGate")}
      </Button>
    {/if}

    <h1 tabindex="-1" data-route-heading class="gs-headline" data-testid="parent_headline">
      {headlineText}
    </h1>

    <div class="gs-meter-card">
      <Meter
        level={meterLevel}
        threshold={connected ? $parent.babyThreshold : null}
        label={meterLabel}
        testid="parent_meter"
      />
    </div>

    <div class="gs-chip-row">
      {#each [...fixed, ...shownExtras] as chip (chip.id)}
        <Chip tone={chip.tone} icon={chip.icon} label={$t(chip.key, chip.args)} testid={chip.id} />
      {/each}
      {#if hiddenCount > 0}
        <Chip
          label={$t("native.ui.common.moreCount", { count: hiddenCount })}
          testid="parent_more_chip"
        />
      {/if}
    </div>

    {#if canRequestSilence($parent)}
      <div class="gs-chip-row">
        <Button
          variant="text"
          testid="sheet_silence_apply"
          disabled={$parent.silenceRequest === "pending"}
          onclick={() => controller.requestSilenceBaby()}
        >
          {$parent.silenceRequest === "pending"
            ? $t("native.ui.silence.pending")
            : $t("native.ui.silence.apply")}
        </Button>
        {#if $parent.silenceRequest === "failed"}
          <span class="gs-hint" role="status" data-testid="sheet_silence_failed">
            {$t("native.ui.silence.failed")}
          </span>
        {/if}
      </div>
    {/if}

    <p class="gs-note" data-testid="parent_keep_open_banner">
      <Glyph name="info" />
      <span>{$t("webScreens.parent.keepOpen")}</span>
    </p>

    <div class="gs-fill"></div>

    <ListenButton
      mode={$parent.listen}
      enabled={connected}
      onchange={(mode) => controller.setListen(mode)}
      testid="parent_listen"
    />

    <div class="gs-action-row">
      <Button variant="outline" testid="parent_snooze" onclick={() => (snoozeOpen = true)}>
        {$t("native.ui.snooze.button")}
      </Button>
      <Button variant="outline" testid="parent_probe" onclick={() => (probeOpen = true)}>
        {$t("native.ui.probe.button")}
      </Button>
      <!-- The night screen button (7.2) joins this row. -->
      <Button variant="outline" testid="parent_overflow" onclick={() => (moreOpen = true)}>
        {$t("native.ui.common.more")}
      </Button>
    </div>
  </div>
{/if}

<Dialog bind:open={snoozeOpen} title={$t("native.ui.snooze.title")} testid="sheet_snooze">
  <p>{$t("native.ui.snooze.help")}</p>
  {#snippet actions()}
    <Button variant="text" onclick={() => (snoozeOpen = false)}>
      {$t("native.ui.common.cancel")}
    </Button>
    {#if snoozeActive}
      <Button
        variant="outline"
        testid="sheet_snooze_end"
        onclick={() => {
          controller.cancelSnooze();
          snoozeOpen = false;
        }}
      >
        {$t("native.ui.snooze.end")}
      </Button>
    {/if}
    <Button variant="tonal" testid="sheet_snooze_10" onclick={() => snooze(10)}>
      {$t("native.ui.snooze.minutes", { minutes: 10 })}
    </Button>
    <Button variant="tonal" testid="sheet_snooze_30" onclick={() => snooze(30)}>
      {$t("native.ui.snooze.minutes", { minutes: 30 })}
    </Button>
  {/snippet}
</Dialog>

<Dialog bind:open={probeOpen} title={$t("native.ui.probe.title")} testid="sheet_probe">
  <p>{$t("native.ui.probe.body")}</p>
  {#snippet actions()}
    <Button variant="text" onclick={() => (probeOpen = false)}>
      {$t("native.ui.common.cancel")}
    </Button>
    <Button testid="sheet_probe_run" onclick={runTest}>{$t("native.ui.probe.run")}</Button>
  {/snippet}
</Dialog>

<Dialog
  bind:open={questionOpen}
  title={$t("native.ui.probe.question")}
  testid="sheet_probe_question"
>
  {#if localOnly}<p>{$t("native.ui.probe.localOnly")}</p>{/if}
  {#snippet actions()}
    <Button variant="text" testid="sheet_probe_no" onclick={() => answer(false)}>
      {$t("native.ui.common.no")}
    </Button>
    <Button testid="sheet_probe_yes" onclick={() => answer(true)}>
      {$t("native.ui.common.yes")}
    </Button>
  {/snippet}
</Dialog>

<Dialog
  bind:open={helpOpen}
  title={$t("native.ui.help.reconnect.title")}
  testid="sheet_help_reconnect"
>
  <ol>
    {#each steps as step (step)}
      <li>{$t(step)}</li>
    {/each}
  </ol>
  {#snippet actions()}
    <Button variant="text" onclick={() => (helpOpen = false)}>{$t("native.ui.common.ok")}</Button>
  {/snippet}
</Dialog>

<Dialog bind:open={moreOpen} title={$t("native.ui.common.more")} testid="sheet_more">
  <div class="gs-stack gs-stack--tight">
    <Button
      variant="outline"
      block
      testid="parent_settings"
      onclick={() => {
        moreOpen = false;
        router.navigate("/settings");
      }}
    >
      {$t("native.ui.common.settings")}
    </Button>
    <Button
      variant="danger"
      block
      testid="parent_stop"
      onclick={() => {
        moreOpen = false;
        stopOpen = true;
      }}
    >
      {$t("native.ui.parent.stop")}
    </Button>
  </div>
  {#snippet actions()}
    <Button variant="text" onclick={() => (moreOpen = false)}
      >{$t("native.ui.common.cancel")}</Button
    >
  {/snippet}
</Dialog>

<Dialog bind:open={stopOpen} title={$t("native.ui.stop.title")} testid="dialog_stop">
  <p>{$t("native.ui.stop.parentBodyDefault")}</p>
  {#snippet actions()}
    <Button variant="text" testid="dialog_stop_cancel" onclick={() => (stopOpen = false)}>
      {$t("native.ui.stop.cancel")}
    </Button>
    <Button variant="danger" testid="dialog_stop_confirm" onclick={stop}>
      {$t("native.ui.stop.confirm")}
    </Button>
  {/snippet}
</Dialog>
