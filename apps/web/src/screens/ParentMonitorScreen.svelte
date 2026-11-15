<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { useController } from "../lib/contract";
  import type { MessageKey } from "../lib/i18n/keys";
  import { locale, t } from "../lib/i18n/store";
  import { useRouter } from "../lib/router-context";
  import {
    askForNotifications,
    browserNotifications,
    flashTitle,
    notifyAway,
  } from "../lib/ui/attention";
  import Button from "../lib/ui/Button.svelte";
  import BannerStack from "../lib/ui/BannerStack.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
  import { formatClock, formatDuration, formatDurationWords, levelWordKey } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import AwayNotice from "../lib/ui/AwayNotice.svelte";
  import KeepAwakeNotice from "../lib/ui/KeepAwakeNotice.svelte";
  import ListenButton from "../lib/ui/ListenButton.svelte";
  import RestoredCard from "../lib/ui/RestoredCard.svelte";
  import SoundStrip from "../lib/ui/SoundStrip.svelte";
  import StopDialog from "../lib/ui/StopDialog.svelte";
  import TalkButton from "../lib/ui/TalkButton.svelte";
  import TalkStatus from "../lib/ui/TalkStatus.svelte";
  import type { TalkHint } from "../lib/ui/talk-view";
  import NightScreen from "../lib/ui/NightScreen.svelte";
  import {
    canRequestSilence,
    connectionName,
    fullScreenOf,
    headlineOf,
    pruneDismissed,
    silenceStateKey,
    type FullScreenKind,
  } from "../lib/ui/parent-view";
  import { everySecond } from "../lib/ui/ticker";
  import BatteryAlertScreen from "./BatteryAlertScreen.svelte";
  import LinkAlarmScreen from "./LinkAlarmScreen.svelte";
  import NoiseAlertScreen from "./NoiseAlertScreen.svelte";

  const controller = useController();
  const router = useRouter();
  const { app, parent, settings, soundUnlocked, webTalk } = controller;

  // ---- full screens ----------------------------------------------------------------------

  let dismissed = $state<ReadonlySet<FullScreenKind>>(new Set());
  $effect(() => {
    const before = untrack(() => dismissed);
    const after = pruneDismissed($parent, before);
    if (after !== before) dismissed = after;
  });
  const fullScreen = $derived(fullScreenOf($parent, dismissed));

  // An alert replaces the night screen at once; it stays off afterwards, the parent is awake now.
  $effect(() => {
    if (fullScreen !== null) nightOpen = false;
  });

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
      case "cant_hear":
        return $t("native.ui.monitor.cantHear");
      case "no_contact":
        return $t("native.ui.parent.noContactYet");
      case "monitoring":
        return $t("native.ui.parent.monitoringNow");
    }
  });

  const micSilent = $derived(connected && $parent.peer.micSilent === true);
  const stripLabel = $derived(
    !connected
      ? $t("native.ui.monitor.noSound")
      : micSilent
        ? $t("native.ui.monitor.micSilentStrip")
        : $t("native.ui.parent.meterA11y", { level: $t(levelWordKey($parent.level)) }),
  );
  // "Monitoring for 1 h 27 min": from the run start the platform reports, while connected.
  const monitoringFor = $derived(
    connected && $parent.monitoringSinceElapsedMs !== null
      ? formatDurationWords(
          $t,
          Math.max(0, $everySecond.elapsedMs - $parent.monitoringSinceElapsedMs),
        )
      : null,
  );

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
    if (m.count === null || !m.complete) {
      // A first-trust report has no gap, so both clock times match: name the time once.
      return args.start === args.end
        ? $t("native.ui.missed.unknownAt", { time: args.end })
        : $t("native.ui.missed.unknown", args);
    }
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

  // ---- talk-back permission step ---------------------------------------------------------

  // The microphone is asked for in a step of its own before the first talk, never during a hold.
  let permissionOpen = $state(false);
  $effect(() => {
    permissionOpen = $parent.talkPermission !== "none";
  });

  // ---- sheets ----------------------------------------------------------------------------

  let snoozeOpen = $state(false);
  let probeOpen = $state(false);
  let moreOpen = $state(false);
  let stopOpen = $state(false);
  let nightOpen = $state(false);
  let helpOpen = $state(false);
  let questionOpen = $state(false);
  let localOnly = $state(false);
  let awaitingTest = $state(false);

  // The pause can be undone for a few seconds, but only when it starts from the monitor; a pause
  // started from a running alert (the alert screen) is final. The window is presentation: it ends
  // when a noise event starts, the pause ends or the time is up.
  const UNDO_MS = 8_000;
  let undoMinutes = $state<number | null>(null);
  let undoTimer: ReturnType<typeof setTimeout> | null = null;
  function clearUndo(): void {
    if (undoTimer !== null) clearTimeout(undoTimer);
    undoTimer = null;
    undoMinutes = null;
  }
  onDestroy(clearUndo);
  $effect(() => {
    if (undoMinutes !== null && ($parent.noiseAlertActive || !snoozeActive)) clearUndo();
  });

  function snooze(minutes: number): void {
    const noiseRunning = $parent.noiseAlertActive;
    controller.snooze(minutes);
    snoozeOpen = false;
    clearUndo();
    if (!noiseRunning) {
      undoMinutes = minutes;
      undoTimer = setTimeout(clearUndo, UNDO_MS);
    }
  }

  function undoSnooze(): void {
    controller.cancelSnooze();
    clearUndo();
  }

  let talkHint = $state<TalkHint | null>(null);

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
{:else if fullScreen === "battery_critical"}
  <BatteryAlertScreen onok={() => dismiss("battery_critical")} />
{:else if fullScreen !== null}
  <LinkAlarmScreen kind={fullScreen} onok={() => dismiss(fullScreen)} />
{:else}
  <div class="gs-screen gs-screen--monitor" data-screen="parent_monitor">
    <!-- Tool row: the DOM order is the tab order (night, then settings); the gear is laid out first
         and stays fixed. The night-mode control of group 6 and the alert chip (Android only) have
         no web UI here. -->
    <div class="gs-tools" data-testid="parent_tools">
      <button
        type="button"
        class="gs-chip-btn"
        data-testid="parent_night_open"
        onclick={() => (nightOpen = true)}
      >
        <Glyph name="moon" />
        <span>{$t("webScreens.night.open")}</span>
      </button>
      <button
        type="button"
        class="gs-icon-btn gs-tools__gear"
        data-testid="tool_settings"
        aria-label={$t("native.ui.common.settings")}
        onclick={() => router.navigate("/settings")}
      >
        <Glyph name="settings" />
      </button>
    </div>

    <BannerStack ontest={() => (probeOpen = true)} />

    <AwayNotice />
    <KeepAwakeNotice />

    {#if $parent.restoredAfterMs !== null}
      <!-- Until OK: the card never dismisses itself, a new outage replaces it. -->
      <RestoredCard restoredAfterMs={$parent.restoredAfterMs} />
    {:else if missedText}
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

    <div class="gs-hero">
      <h1 tabindex="-1" data-route-heading class="gs-headline" data-testid="parent_headline">
        {headlineText}
      </h1>
      {#if monitoringFor !== null}
        <p class="gs-hero__sub" data-testid="parent_monitoring_for">
          {$t("native.ui.monitor.monitoringFor", { duration: monitoringFor })}
        </p>
      {/if}
    </div>

    {#if $parent.link.phase === "connecting"}
      <!-- Wrong place: before the first connection of a run, one tap at levels 0 and 1. From level
           2 the full screen has the same button, behind the stop confirmation. -->
      <Button
        variant="text"
        testid="parent_choose_other_connection"
        onclick={() => {
          controller.chooseAnotherConnection();
          router.navigate("/");
        }}
      >
        {$t("native.ui.connAlert.wrongConnection")}
      </Button>
    {/if}
    {#if !connected && ($parent.link.phase === "connecting" || $parent.link.phase === "reconnecting")}
      <Button variant="text" testid="parent_help_reconnect" onclick={() => (helpOpen = true)}>
        {$t("native.ui.help.reconnect.open")}
      </Button>
    {/if}

    <SoundStrip
      level={$parent.level}
      {connected}
      threshold={$parent.babyThreshold}
      {micSilent}
      label={stripLabel}
      testid="parent_meter"
    />

    {#if connected || $settings.lastTestWallMs !== null}
      <p class="gs-meta" data-testid="parent_meta">
        {#if connected}
          <span data-testid="parent_silence_state">{$t(silenceStateKey($parent.peer))}</span>
        {/if}
        {#if $settings.lastTestWallMs !== null}
          <span data-testid="parent_last_test">
            {$t("native.ui.monitor.alertsTested", {
              time: formatClock($settings.lastTestWallMs, $locale),
            })}
          </span>
        {/if}
      </p>
    {/if}

    <p class="gs-note" data-testid="parent_keep_open_banner">
      <Glyph name="info" />
      <span>{$t("webScreens.parent.keepOpen")}</span>
    </p>

    <div class="gs-fill"></div>

    {#if connected && !$settings.talkCoachSeen && !$app.coachPending}
      <Card tone="variant" testid="coach_talk">
        <p>{$t("webScreens.talk.coach")}</p>
        <Button
          variant="text"
          testid="coach_talk_dismiss"
          onclick={() => controller.dismissTalkCoach()}
        >
          {$t("native.ui.common.ok")}
        </Button>
      </Card>
    {/if}

    {#if undoMinutes !== null}
      <div class="gs-snackbar" role="status" data-testid="parent_pause_snackbar">
        <span>{$t("native.ui.snooze.snackbar", { minutes: undoMinutes })}</span>
        <Button variant="text" testid="parent_pause_undo" onclick={undoSnooze}>
          {$t("native.ui.snooze.undo")}
        </Button>
      </div>
    {/if}

    <TalkStatus
      talk={$parent}
      web={$webTalk}
      nowMs={$everySecond.elapsedMs}
      hint={talkHint}
      onkeeplisten={() => controller.setListen("on")}
    />

    <div class="gs-dock">
      <ListenButton
        mode={$parent.listen}
        enabled={connected}
        onchange={(mode) => controller.setListen(mode)}
        testid="parent_listen"
      />
      <div class="gs-dock__tiles">
        <TalkButton
          talk={$parent}
          web={$webTalk}
          nowMs={$everySecond.elapsedMs}
          nowWallMs={$everySecond.wallMs}
          bind:hint={talkHint}
          onpress={() => controller.setTalk(true)}
          onrelease={() => controller.setTalk(false)}
        />
        <button
          type="button"
          class="gs-tile"
          data-kind="pause"
          data-paused={snoozeActive || undefined}
          data-testid="parent_snooze"
          aria-label={$t("native.ui.snooze.button")}
          onclick={() => (snoozeOpen = true)}
        >
          <Glyph name="snooze" />
          <span class="gs-tile__text">
            <span class="gs-tile__label">
              {$t(snoozeActive ? "native.ui.snooze.tilePaused" : "native.ui.snooze.tile")}
            </span>
          </span>
        </button>
        <button
          type="button"
          class="gs-tile"
          data-kind="more"
          data-testid="parent_overflow"
          onclick={() => (moreOpen = true)}
        >
          <Glyph name="more" />
          <span class="gs-tile__text">
            <span class="gs-tile__label">{$t("native.ui.common.more")}</span>
          </span>
        </button>
      </div>
    </div>
    <NightScreen bind:open={nightOpen} statusText={headlineText} />
  </div>
{/if}

<Dialog
  status
  bind:open={permissionOpen}
  title={$t("webScreens.talk.permissionTitle")}
  testid="parent_talk_permission_sheet"
  onclose={() => controller.dismissTalkPermission()}
>
  {#if $parent.talkPermission === "just_granted"}
    <p>{$t("webScreens.talk.permissionDone")}</p>
  {:else if $parent.talkPermission === "refused"}
    <p>{$t("webScreens.talk.permissionRefused")}</p>
  {:else}
    <p>{$t("webScreens.talk.permissionBody")}</p>
  {/if}
  {#snippet actions()}
    {#if $parent.talkPermission === "needed"}
      <Button
        variant="text"
        testid="parent_talk_permission_cancel"
        onclick={() => controller.dismissTalkPermission()}
      >
        {$t("native.ui.common.cancel")}
      </Button>
      <Button
        testid="parent_talk_permission_allow"
        onclick={() => controller.requestTalkPermission()}
      >
        {$t("webScreens.talk.permissionAllow")}
      </Button>
    {:else}
      <Button testid="parent_talk_permission_ok" onclick={() => controller.dismissTalkPermission()}>
        {$t("native.ui.common.ok")}
      </Button>
    {/if}
  {/snippet}
</Dialog>

<Dialog status bind:open={snoozeOpen} title={$t("native.ui.snooze.title")} testid="sheet_snooze">
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

<Dialog status bind:open={probeOpen} title={$t("native.ui.probe.title")} testid="sheet_probe">
  <p>{$t("native.ui.probe.body")}</p>
  {#snippet actions()}
    <Button variant="text" onclick={() => (probeOpen = false)}>
      {$t("native.ui.common.cancel")}
    </Button>
    <Button testid="sheet_probe_run" onclick={runTest}>{$t("native.ui.probe.run")}</Button>
  {/snippet}
</Dialog>

<Dialog
  status
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
  status
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

<Dialog status bind:open={moreOpen} title={$t("native.ui.common.more")} testid="sheet_more">
  <div class="gs-stack gs-stack--tight">
    <Button
      variant="outline"
      block
      testid="parent_probe"
      onclick={() => {
        moreOpen = false;
        probeOpen = true;
      }}
    >
      {$t("native.ui.probe.button")}
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

<!-- One stop dialog on screen at a time: the connection alert brings its own. -->
{#if fullScreen === null}
  <StopDialog bind:open={stopOpen} name={connectionName($app.peerName)} onconfirm={stop} />
{/if}
