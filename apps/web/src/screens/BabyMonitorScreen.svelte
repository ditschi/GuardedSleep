<script lang="ts">
  import { useController, type LinkUi } from "../lib/contract";
  import type { MessageKey } from "../lib/i18n/keys";
  import { locale, t } from "../lib/i18n/store";
  import { link } from "../lib/router";
  import { useRouter } from "../lib/router-context";
  import Banner from "../lib/ui/Banner.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Chip from "../lib/ui/Chip.svelte";
  import Dialog from "../lib/ui/Dialog.svelte";
  import { formatClock, formatDuration, levelWordKey } from "../lib/ui/format";
  import Glyph from "../lib/ui/Glyph.svelte";
  import AwayNotice from "../lib/ui/AwayNotice.svelte";
  import { askForNotifications, browserNotifications } from "../lib/ui/attention";
  import KeepAwakeNotice from "../lib/ui/KeepAwakeNotice.svelte";
  import type { GlyphName, Tone } from "../lib/ui/glyphs";
  import Meter from "../lib/ui/Meter.svelte";
  import { babyFailureKey, coreMessageKey, isMicFailure } from "../lib/ui/messages";
  import NightScreen from "../lib/ui/NightScreen.svelte";
  import ScreenHeader from "../lib/ui/ScreenHeader.svelte";

  const controller = useController();
  const router = useRouter();
  const { baby, deviceBattery, pageGuard, babyTalk } = controller;

  let stopOpen = $state(false);
  let nightOpen = $state(false);

  interface StatusLook {
    key: MessageKey;
    tone: Tone;
    icon: GlyphName;
  }

  // The baby device never alerts; it says in one line whether the parent device is reachable.
  function statusOf(l: LinkUi): StatusLook {
    const reason = babyFailureKey(l.failureKey);
    if (l.phase === "failed_needs_user") {
      return {
        key: reason ?? coreMessageKey("pairing_key_mismatch") ?? "native.ui.baby.status.waiting",
        tone: "bad",
        icon: "alert",
      };
    }
    switch (l.phase) {
      case "connected":
        return { key: "native.ui.baby.status.connected", tone: "ok", icon: "link" };
      case "peer_stopped":
        return { key: "native.ui.baby.status.parentStopped", tone: "neutral", icon: "pause" };
      case "reconnecting":
      case "connecting":
        if (l.level >= 2) {
          return { key: "native.ui.baby.status.notResponding", tone: "warn", icon: "linkOff" };
        }
        return l.phase === "reconnecting"
          ? { key: "native.ui.baby.status.reconnecting", tone: "warn", icon: "refresh" }
          : { key: "native.ui.baby.status.waiting", tone: "neutral", icon: "refresh" };
      default:
        return { key: "native.ui.baby.status.waiting", tone: "neutral", icon: "refresh" };
    }
  }

  const status = $derived(statusOf($baby.link));
  // A run that could not start (microphone refused) says why, and can be tried again.
  const micFailure = $derived(isMicFailure($baby.link.failureKey));
  const failureText = $derived.by(() => {
    const key = micFailure ? babyFailureKey($baby.link.failureKey) : null;
    return key === null ? null : $t(key);
  });

  const batteryText = $derived.by(() => {
    const b = $deviceBattery;
    if (b === null) return null;
    return b.charging === true
      ? $t("native.ui.baby.batteryCharging", { percent: b.pct })
      : $t("native.ui.baby.battery", { percent: b.pct });
  });

  const lastTalkText = $derived.by(() => {
    const last = $babyTalk.lastTalk;
    if (last === null) return null;
    return $t("webScreens.talk.babyLastTalk", {
      name: last.name === "" ? $t("native.ui.common.parentUnitDefault") : last.name,
      time: formatClock(last.atWallMs, $locale),
      duration: formatDuration($t, last.durationMs),
    });
  });

  function openNight(): void {
    nightOpen = true;
    // A tap is the moment to ask for notifications; nothing changes if the user declines.
    void askForNotifications(browserNotifications(window));
  }

  function stop(): void {
    stopOpen = false;
    controller.stopMonitoring();
    router.navigate("/");
  }
</script>

<div class="gs-screen" data-screen="baby_monitor">
  <ScreenHeader title={$t("native.ui.common.babyUnitDefault")}>
    {#snippet actions()}
      <a
        class="gs-icon-btn"
        href="/settings"
        use:link={router}
        data-testid="baby_settings"
        aria-label={$t("layout.settingsAria")}
      >
        <Glyph name="settings" />
      </a>
    {/snippet}
  </ScreenHeader>

  <section class="gs-status-card" data-tone={status.tone} aria-live="polite">
    <div class="gs-status-card__head">
      <Glyph name={status.icon} />
      <h1 tabindex="-1" data-route-heading data-testid="baby_status" data-state={$baby.link.phase}>
        {$t(status.key)}
      </h1>
    </div>
    <p class="gs-muted" data-testid="baby_monitoring_line">{$t("native.ui.baby.monitoring")}</p>
    {#if $baby.parentListening}
      <p class="gs-muted" data-testid="baby_listening">{$t("native.ui.baby.parentListening")}</p>
    {/if}
  </section>

  {#if $babyTalk.talking}
    <p class="gs-muted" role="status" data-testid="baby_talking">
      <Glyph name="mic" />
      {$t("webScreens.talk.babyTalking")}
    </p>
  {/if}
  {#if $babyTalk.soundSuspended}
    <Button variant="tonal" testid="baby_sound_off" onclick={() => controller.unlockSound()}>
      <Glyph name="sound" />
      {$t("webScreens.talk.babySoundOff")}
    </Button>
  {/if}
  {#if lastTalkText}
    <p class="gs-muted" data-testid="baby_last_talk">{lastTalkText}</p>
  {/if}

  {#if failureText}
    <div class="gs-stack gs-stack--tight">
      <Banner tone="bad" message={failureText} testid="baby_failure" />
      <Button variant="tonal" testid="baby_retry" onclick={() => controller.startMonitoring()}>
        {$t("native.ui.common.tryAgain")}
      </Button>
    </div>
  {/if}

  <AwayNotice />
  <KeepAwakeNotice />

  {#if $baby.micSilent}
    <Banner tone="warn" message={$t("native.ui.baby.micSilent")} testid="baby_mic_silent" />
  {/if}

  <div class="gs-meter-block">
    <span>{$t("native.ui.baby.micLevel")}</span>
    <Meter
      compact
      level={$baby.micLevel}
      threshold={$baby.threshold}
      label={$t("native.ui.parent.meterA11y", { level: $t(levelWordKey($baby.micLevel)) })}
      testid="baby_mic_level"
    />
  </div>

  <div class="gs-chip-row">
    {#if batteryText}
      <Chip icon="battery" label={batteryText} testid="baby_battery" />
    {/if}
    <Chip icon="info" label={$t("webScreens.baby.quietUnknown")} testid="baby_silence" />
    {#if $pageGuard.wakeLock === "held"}
      <Chip
        tone="ok"
        icon="sun"
        label={$t("webScreens.guard.wakeLockHeld")}
        testid="baby_wake_lock"
      />
    {/if}
  </div>

  <div class="gs-checklist" data-testid="baby_keep_awake">
    <div>
      <Glyph name="info" />
      <span>{$t("webScreens.baby.keepAwake")}</span>
    </div>
    <div>
      <Glyph name="info" />
      <span>{$t("webScreens.baby.keepSilent")}</span>
    </div>
    <div>
      <Glyph name="sound" />
      <span data-testid="baby_talk_volume_hint">{$t("webScreens.talk.babyVolumeHint")}</span>
    </div>
    {#if $babyTalk.settings.enabled}
      <div>
        <Glyph name="mic" />
        <span data-testid="baby_talk_checklist">{$t("webScreens.talk.babyChecklist")}</span>
      </div>
    {/if}
  </div>

  <div class="gs-fill"></div>

  <Button variant="tonal" block size="large" testid="baby_night_open" onclick={openNight}>
    <Glyph name="moon" />
    {$t("webScreens.night.open")}
  </Button>

  <Button variant="outline" block testid="baby_stop" onclick={() => (stopOpen = true)}>
    {$t("native.ui.baby.stop")}
  </Button>
</div>

<NightScreen
  bind:open={nightOpen}
  statusText={$t(status.key)}
  talkingText={$babyTalk.talking ? $t("webScreens.talk.babyTalking") : null}
  {lastTalkText}
  soundOffText={$babyTalk.soundSuspended ? $t("webScreens.talk.babySoundOff") : null}
  onsoundon={() => controller.unlockSound()}
/>

<Dialog bind:open={stopOpen} title={$t("native.ui.stop.title")} testid="dialog_stop">
  <p>{$t("native.ui.stop.babyBody")}</p>
  {#snippet actions()}
    <Button variant="text" testid="dialog_stop_cancel" onclick={() => (stopOpen = false)}>
      {$t("native.ui.stop.cancel")}
    </Button>
    <Button variant="danger" testid="dialog_stop_confirm" onclick={stop}>
      {$t("native.ui.stop.confirm")}
    </Button>
  {/snippet}
</Dialog>
