<script lang="ts">
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { onDestroy, onMount } from "svelte";
  import {
    attachAlertAudioPipeline,
    type AlertAudioPipeline,
  } from "$lib/audio/parent-alert-audio.js";
  import { sampleStreamRms } from "$lib/audio/sample-stream-rms.js";
  import AmbientBar from "$lib/components/AmbientBar.svelte";
  import ListenButton from "$lib/components/ListenButton.svelte";
  import type { GuardedsleepDevTestApi } from "$lib/dev-e2e-bridge.js";
  import { getI18n } from "$lib/i18n/context.js";
  import { alarmModeFor, AlarmTone } from "$lib/link/alarm-tone.js";
  import { formatClockTime, formatDuration } from "$lib/link/format.js";
  import { buildLinkConfig } from "$lib/link/link-config.js";
  import { getCore } from "$lib/link/core-loader.js";
  import { meterLevel, meterWord } from "$lib/link/monitor-state.js";
  import { ParentMonitor } from "$lib/link/parent-monitor.js";
  import { initialParentState, type ParentMonitorState } from "$lib/link/ui-contract.js";
  import { createWakeLock, trackVisibility } from "$lib/link/web-readiness.js";
  import { getActivePairing, markPairedBefore } from "$lib/pairing/pairing-memory.js";
  import { pushBackInterceptor } from "$lib/ui/app-navigation.js";

  const i18n = getI18n();
  const t = (key: string, params?: Record<string, string | number>): string =>
    i18n?.t(key, params) ?? key;

  const pairing = browser ? getActivePairing() : null;

  let monitor: ParentMonitor | undefined = $state();
  let st: ParentMonitorState = $state(initialParentState());
  let nowElapsed = $state(0);
  let nowWall = $state(Date.now());
  let startedElapsed = $state(0);
  let audioEl: HTMLAudioElement | undefined = $state();
  let remoteStream: MediaStream | null = $state(null);
  let soundUnlocked = $state(false);
  let started = $state(false);
  let loadError = $state(false);
  let sheet: "snooze" | "probe" | "stop" | "help" | null = $state(null);
  let testQuestionAt: number | null = $state(null);
  let testAnswer: "yes" | "no" | null = $state(null);
  let backgroundWarning = $state(false);
  let wasHiddenNow = $state(false);
  let suspendWarning = $state(false);

  const alarm = new AlarmTone();
  const wake = createWakeLock();
  const wakeActive = wake.active;
  const visibility = browser ? trackVisibility() : null;
  let pipeline: AlertAudioPipeline | undefined;
  let noiseAlertTotal = 0;
  let noiseWaiter: ((rms: number) => void) | null = null;
  const unsubs: (() => void)[] = [];

  const listenInfo = $derived(st.listen);
  const connected = $derived(st.link.phase === "connected");
  const level = $derived(st.link.level);
  const snoozeActive = $derived(st.snoozedUntilWallMs != null && st.snoozedUntilWallMs > nowWall);
  const alarmMuted = $derived(
    st.link.acknowledgedUntilElapsedMs != null && st.link.acknowledgedUntilElapsedMs > nowElapsed,
  );
  const lostAlarm = $derived(
    (st.link.phase === "connecting" || st.link.phase === "reconnecting") && level >= 2,
  );
  const soundPlays = $derived(st.alert != null && !st.alert.soundStopped);
  /** Wall time of the last verified contact; null before the first contact. */
  const lastContactWallMs = $derived(
    st.link.lastAliveElapsedMs != null ? nowWall - (nowElapsed - st.link.lastAliveElapsedMs) : null,
  );
  /** Peer chips show the state from before an outage: dim them and say when it was. */
  const stale = $derived(!connected && lastContactWallMs != null);
  const atTime = $derived(
    stale && lastContactWallMs != null
      ? ` ${t("web.common.atTime", { time: formatClockTime(lastContactWallMs, i18n?.locale) })}`
      : "",
  );
  const UNIT_WARNING_KEYS: Record<string, string> = {
    page_hidden: "web.unitWarning.pageHidden",
    wake_lock_off: "web.unitWarning.wakeLockOff",
    resumed_from_sleep: "web.unitWarning.resumedFromSleep",
    not_charging: "web.unitWarning.notCharging",
    mic_permission_lost: "web.unitWarning.micPermissionLost",
  };
  const thresholdPct = $derived(
    st.peer.noiseThreshold != null ? meterLevel(st.peer.noiseThreshold) * 100 : null,
  );

  onMount(() => {
    if (!pairing) return;
    // Autoplay allowed (installed PWA, user setting, test browsers): start without a tap.
    const probe = new AlarmTone();
    void probe.unlock().then((ok) => {
      probe.destroy();
      if (ok) void unlockAndStart();
    });

    const clock = setInterval(() => {
      nowWall = Date.now();
      if (monitor) nowElapsed = monitor.link.now();
    }, 1000);
    const offBack = pushBackInterceptor(() => {
      if (sheet) {
        sheet = null;
        return true;
      }
      return false;
    });
    const onVisibility = (): void => {
      if (document.visibilityState === "visible") {
        void audioEl?.play().catch(() => {});
        wasHiddenNow = false;
      } else {
        wasHiddenNow = true;
      }
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      clearInterval(clock);
      offBack();
      document.removeEventListener("visibilitychange", onVisibility);
    };
  });

  async function unlockAndStart(): Promise<void> {
    if (started || !pairing) return;
    started = true;
    soundUnlocked = await alarm.unlock();
    if (typeof Notification !== "undefined" && Notification.permission === "default") {
      void Notification.requestPermission().catch(() => {});
    }
    let core;
    try {
      core = await getCore();
    } catch {
      loadError = true;
      started = false;
      return;
    }
    const m = new ParentMonitor({
      core,
      config: buildLinkConfig("parent", pairing.pskB64Url, pairing.pairedBefore),
      signalingUrls: pairing.signalingUrls,
      onRemoteStream: () => {},
    });
    unsubs.push(m.state.subscribe((s) => (st = s)));
    unsubs.push(m.remoteStream.subscribe((s) => (remoteStream = s)));
    unsubs.push(
      m.onLinkEvent((ev) => {
        if (ev.type === "status" && ev.status.state === "connected") markPairedBefore();
        if (ev.type === "noise_alert" && !ev.snoozed) onNoise(ev.probe, ev.rms);
        if (ev.type === "noise_alert") {
          noiseAlertTotal += 1;
          const w = noiseWaiter;
          noiseWaiter = null;
          w?.(ev.rms);
        }
      }),
    );
    unsubs.push(
      m.suspendGap.subscribe((g) => {
        if (g) suspendWarning = true;
      }),
    );
    monitor = m;
    startedElapsed = m.link.now();
    nowElapsed = startedElapsed;
    m.start();
    wake.start();
  }

  function onNoise(probe: boolean, _rms: number): void {
    try {
      navigator.vibrate?.([180, 120, 180]);
    } catch {
      /* not supported */
    }
    if (probe) testQuestionAt = Date.now() + 5000;
    if (document.visibilityState === "hidden") {
      try {
        if (typeof Notification !== "undefined" && Notification.permission === "granted") {
          new Notification(t("web.alert.title"));
        }
      } catch {
        /* ignore */
      }
    }
  }

  // Alert audio: play the incoming stream through the gain pipeline while an event sounds.
  $effect(() => {
    const el = audioEl;
    const stream = remoteStream;
    if (!el || !stream) return;
    el.srcObject = stream;
    el.muted = false;
    void el.play().catch(() => {});
  });

  $effect(() => {
    const el = audioEl;
    const stream = remoteStream;
    const on = soundPlays;
    if (!el || !stream) return;
    if (on && !pipeline) {
      void attachAlertAudioPipeline(stream, el).then((p) => {
        if (soundPlays) pipeline = p;
        else p.disconnect();
      });
    } else if (!on && pipeline) {
      pipeline.disconnect();
      pipeline = undefined;
    }
    el.muted = st.alert?.soundStopped === true;
  });

  // Connection alarm: soft beep every 10 s at level 2, continuous at level 3, off when muted.
  $effect(() => {
    if (!soundUnlocked) return;
    alarm.setMode(alarmModeFor(lostAlarm ? level : 0, alarmMuted));
  });

  // Page title flashes while something needs attention.
  $effect(() => {
    if (!browser) return;
    const attention =
      st.alert != null ? t("web.alert.title") : lostAlarm ? t("web.bar.lostTitle") : null;
    if (attention == null) {
      document.title = t("meta.appTitle");
      return;
    }
    let on = true;
    const id = setInterval(() => {
      on = !on;
      document.title = on ? attention : t("meta.appTitle");
    }, 1000);
    document.title = attention;
    return () => clearInterval(id);
  });

  // Hidden-tab warning on return.
  $effect(() => {
    if (!visibility) return;
    return visibility.state.subscribe((v) => {
      if (v.returnedAfterHidden && started) backgroundWarning = true;
    });
  });

  function setListening(on: boolean): void {
    monitor?.setListen(on ? (listenInfo === "locked" ? "locked" : "hold") : "off");
  }
  function setListenLocked(locked: boolean): void {
    monitor?.setListen(locked ? "locked" : listenInfo === "off" ? "off" : "hold");
  }

  function stopMonitoring(): void {
    sheet = null;
    monitor?.stop();
    wake.stop();
    alarm.setMode("off");
    started = false;
    void goto("/");
  }

  function answerTest(a: "yes" | "no"): void {
    testAnswer = a;
    testQuestionAt = null;
  }

  $effect(() => {
    if (testQuestionAt == null) return;
    const id = setInterval(() => (nowWall = Date.now()), 500);
    return () => clearInterval(id);
  });

  $effect(() => {
    if (!import.meta.env.DEV || !browser || !monitor) return;
    const m = monitor;
    const api: GuardedsleepDevTestApi = {
      role: "parent",
      getSession: () => ({
        getE2ePeerConnection: () => m.link.currentPeer()?.pc ?? null,
        getE2eDataChannel: () => m.link.currentPeer()?.dataChannel() ?? null,
        getE2ePairingVerified: () => m.link.status?.state === "connected",
        getE2eBabyLocalAudioTrackEnabled: () => null,
        sendListen: (on: boolean) => m.setListen(on ? "locked" : "off"),
      }),
      waitNextNoiseAlert: (timeoutMs = 5000) =>
        new Promise<number>((resolve, reject) => {
          const timer = setTimeout(() => {
            noiseWaiter = null;
            reject(new Error("noise alert timeout"));
          }, timeoutMs);
          noiseWaiter = (rms) => {
            clearTimeout(timer);
            resolve(rms);
          };
        }),
      getNoiseAlertCount: () => noiseAlertTotal,
      resetNoiseAlertCount: () => (noiseAlertTotal = 0),
      sampleRemoteAudioRms: (opts) =>
        remoteStream
          ? sampleStreamRms(remoteStream, opts)
          : Promise.resolve({ maxRms: 0, avgRms: 0, samples: 0 }),
      sendProbeTest: () => m.requestTestAlert(),
      getLinkSnapshot: () => ({ state: st, nowElapsed: m.link.now() }),
      resetListenAudioForE2e: async () => {
        pipeline?.disconnect();
        pipeline = undefined;
      },
    };
    window.__guardedsleep_test = api;
    return () => {
      if (window.__guardedsleep_test === api) delete window.__guardedsleep_test;
    };
  });

  onDestroy(() => {
    for (const u of unsubs) u();
    monitor?.stop();
    wake.stop();
    alarm.destroy();
    pipeline?.disconnect();
    visibility?.destroy();
  });

  const levelWord = $derived(meterWord(st.level));
  const snoozeMinutesLeft = $derived(
    st.snoozedUntilWallMs != null
      ? Math.max(1, Math.ceil((st.snoozedUntilWallMs - nowWall) / 60_000))
      : 0,
  );
  const wrapperTone = $derived(
    lostAlarm && level >= 3
      ? "rounded-xl bg-red-900/60"
      : lostAlarm || st.alert != null
        ? "rounded-xl bg-red-950/40"
        : "",
  );
</script>

<audio bind:this={audioEl} autoplay playsinline class="hidden" data-testid="parent_audio"></audio>

<div class={`mx-auto flex max-w-md flex-col gap-4 px-4 py-6 ${wrapperTone}`}>
  <h1 class="text-xl font-semibold text-zinc-50">{t("web.parent.title")}</h1>

  {#if !pairing}
    <div
      class="rounded-xl border border-amber-800 bg-amber-950/30 p-4"
      data-testid="parent_needs_pairing"
    >
      <p class="font-medium text-amber-100">{t("web.reload.title")}</p>
      <p class="mt-1 text-sm text-amber-200/90">{t("web.reload.body")}</p>
      <a
        class="mt-3 inline-flex min-h-12 items-center rounded-lg bg-emerald-600 px-4 font-medium text-emerald-950"
        href="/pair?role=parent"
        data-testid="parent_pair_again">{t("web.reload.action")}</a
      >
    </div>
  {:else}
    {#if loadError}
      <p class="rounded-lg bg-red-950 p-3 text-sm text-red-100" role="alert">
        {t("web.loadError")}
      </p>
    {/if}

    {#if !started}
      <button
        type="button"
        class="min-h-[4.5rem] rounded-xl bg-emerald-600 px-4 text-lg font-semibold text-emerald-950 hover:bg-emerald-500"
        data-testid="parent_sound_gate"
        onclick={() => void unlockAndStart()}
      >
        {t("web.parent.soundGate")}
      </button>
      <p class="text-xs text-zinc-500">{t("web.parent.soundGateHint")}</p>
    {:else if !soundUnlocked}
      <button
        type="button"
        class="min-h-14 rounded-xl bg-amber-500 px-4 font-semibold text-amber-950"
        data-testid="parent_sound_unlock"
        onclick={async () => (soundUnlocked = await alarm.unlock())}
      >
        {t("web.parent.soundGate")}
      </button>
    {/if}

    {#if started}
      <p
        class="rounded-lg border border-amber-800/70 bg-amber-950/30 px-3 py-2 text-sm text-amber-100"
        data-testid="parent_keep_open_banner"
      >
        {t("web.keepOpen.parent")}
        {#if !wake.supported}
          <span class="block text-xs text-amber-200/80">{t("web.keepOpen.noWakeLock")}</span>
        {:else if !$wakeActive}
          <span class="block text-xs text-amber-200/80">{t("web.keepOpen.wakeLockOff")}</span>
        {/if}
      </p>

      {#if backgroundWarning || wasHiddenNow}
        <div
          class="rounded-lg border border-orange-700 bg-orange-950/40 p-3 text-sm text-orange-100"
          role="alert"
          data-testid="parent_background_warning"
        >
          <p>{t("web.background.warning")}</p>
          {#if backgroundWarning && !wasHiddenNow}
            <button
              type="button"
              class="mt-2 min-h-12 rounded-lg bg-white/10 px-4"
              data-testid="parent_background_ok"
              onclick={() => {
                backgroundWarning = false;
                visibility?.dismissReturnWarning();
              }}>{t("web.common.ok")}</button
            >
          {/if}
        </div>
      {/if}
      {#if suspendWarning}
        <div
          class="rounded-lg border border-orange-700 bg-orange-950/40 p-3 text-sm text-orange-100"
          role="alert"
          data-testid="parent_suspend_warning"
        >
          <p>{t("web.suspend.warningParent")}</p>
          <button
            type="button"
            class="mt-2 min-h-12 rounded-lg bg-white/10 px-4"
            onclick={() => (suspendWarning = false)}>{t("web.common.ok")}</button
          >
        </div>
      {/if}

      <AmbientBar
        link={st.link}
        role="parent"
        testid="parent_ambient_bar"
        nowElapsedMs={nowElapsed}
        startedElapsedMs={startedElapsed}
        onTryNow={() => monitor?.tryNow()}
        onChecking={() => monitor?.acknowledgeAlarm()}
      />

      {#if st.link.phase === "failed_needs_user"}
        <div class="rounded-lg bg-red-950 p-3 text-sm text-red-100" data-testid="parent_pair_error">
          <p>
            {st.link.failureKey === "update_required"
              ? t("web.bar.updateRequired")
              : st.link.failureKey === "browser_unsupported"
                ? t("web.bar.browserUnsupported")
                : t("web.pair.error.mismatch")}
          </p>
          {#if st.link.failureKey !== "browser_unsupported"}
            <a class="mt-2 inline-block underline" href="/pair?role=parent"
              >{t("web.reload.action")}</a
            >
          {/if}
        </div>
      {/if}

      {#if st.restoredAfterMs != null}
        <p
          class="rounded-lg bg-emerald-950 px-3 py-2 text-sm text-emerald-100"
          role="status"
          data-testid="parent_restored_toast"
        >
          {t("web.bar.restoredAfter", { duration: formatDuration(st.restoredAfterMs) })}
        </p>
      {/if}

      {#if st.securityWarning}
        <div
          class="rounded-lg border border-red-800 bg-red-950/50 p-3 text-sm text-red-100"
          role="alert"
          data-testid="parent_security_warning"
        >
          <p>{t("web.security.warning")}</p>
          <button
            type="button"
            class="mt-2 min-h-12 rounded-lg bg-white/10 px-4"
            onclick={() => monitor?.dismissSecurityWarning()}>{t("web.common.ok")}</button
          >
        </div>
      {/if}

      {#if st.missed}
        <div
          class="rounded-lg border border-zinc-700 bg-zinc-900 p-3 text-sm"
          data-testid="card_missed_events"
        >
          <p>
            {#if st.missed.complete && st.missed.count != null && st.missed.count > 0}
              {t("web.missed.some", {
                count: st.missed.count,
                duration: formatDuration(st.missed.downMs),
              })}
            {:else if st.missed.complete}
              {t("web.missed.none")}
            {:else}
              {t("web.missed.unknown", { duration: formatDuration(st.missed.downMs) })}
            {/if}
          </p>
          <button
            type="button"
            class="mt-2 min-h-12 rounded-lg bg-white/10 px-4"
            data-testid="card_missed_events_ok"
            onclick={() => monitor?.dismissMissedEvents()}>{t("web.common.ok")}</button
          >
        </div>
      {/if}

      {#if st.link.phase === "peer_stopped"}
        <div
          class="rounded-xl border border-zinc-600 bg-zinc-900 p-4"
          data-testid="link_alarm_stopped"
        >
          <p class="font-medium">{t("web.linkAlarm.stopped.title")}</p>
          <p class="mt-1 text-sm text-zinc-300">{t("web.linkAlarm.stopped.body")}</p>
        </div>
      {/if}

      {#if lostAlarm}
        <div
          class={`rounded-xl border p-4 ${
            level >= 3
              ? "border-red-300 bg-red-800 text-white"
              : "border-red-700 bg-red-950 text-red-50"
          }`}
          role="alert"
          data-testid="link_alarm"
          data-level={level}
        >
          <p
            class={`font-semibold ${level >= 3 ? "text-lg motion-safe:animate-pulse" : ""}`}
            data-testid="link_alarm_title"
          >
            {level >= 3 ? t("web.linkAlarm.lost.titleUrgent") : t("web.linkAlarm.lost.title")}
          </p>
          <p class="mt-1 text-sm" data-testid="link_alarm_body">
            {t("web.linkAlarm.lost.body")}
          </p>
          {#if level >= 3 && !alarmMuted}
            <p class="mt-1 text-sm font-medium" data-testid="link_alarm_sound_note">
              {t("web.linkAlarm.lost.soundNote")}
            </p>
          {/if}
        </div>
      {/if}

      {#if st.alert}
        <div
          class="rounded-xl border border-red-600 bg-red-950 p-4 text-red-50"
          role="alert"
          data-testid="parent_noise_alert"
          data-probe={st.alert.probe ? "true" : "false"}
        >
          <p class="flex items-center gap-2 text-lg font-semibold">
            <span data-testid="alert_title">{t("web.alert.title")}</span>
            {#if st.alert.probe}
              <span class="rounded bg-white/20 px-2 text-xs" data-testid="alert_test_badge"
                >{t("web.alert.testBadge")}</span
              >
            {/if}
          </p>
          <p class="text-sm" data-testid="alert_since">
            {t("web.alert.since", { time: formatClockTime(st.alert.sinceWallMs, i18n?.locale) })}
          </p>
          <div class="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              class="min-h-[4.5rem] flex-1 rounded-xl bg-white px-4 font-semibold text-red-950 disabled:opacity-50"
              data-testid="alert_stop_sound"
              disabled={st.alert.soundStopped}
              onclick={() => monitor?.stopAlertSound()}>{t("web.alert.stopSound")}</button
            >
            <button
              type="button"
              class="min-h-12 rounded-lg bg-white/10 px-3"
              data-testid="alert_snooze_10"
              onclick={() => monitor?.snooze(10)}>{t("web.snooze.minutes", { minutes: 10 })}</button
            >
            <button
              type="button"
              class="min-h-12 rounded-lg bg-white/10 px-3"
              data-testid="alert_snooze_30"
              onclick={() => monitor?.snooze(30)}>{t("web.snooze.minutes", { minutes: 30 })}</button
            >
          </div>
        </div>
      {/if}

      <section class="rounded-xl border border-zinc-800 bg-zinc-900/60 p-4">
        <p class="text-lg font-medium" data-testid="parent_headline">
          {#if st.alert && !st.alert.probe}
            {t("web.headline.soundSince", {
              time: formatClockTime(st.alert.sinceWallMs, i18n?.locale),
            })}
          {:else if st.lastNoiseWallMs != null && st.lastNoiseSnoozed}
            {t("web.headline.soundSnoozed", {
              time: formatClockTime(st.lastNoiseWallMs, i18n?.locale),
            })}
          {:else if connected && st.quietSinceWallMs != null}
            {t("web.headline.allQuietSince", {
              time: formatClockTime(st.quietSinceWallMs, i18n?.locale),
            })}
          {:else if st.link.lastAliveElapsedMs != null && !connected}
            {t("web.headline.lastContact", {
              time: formatClockTime(
                nowWall - (nowElapsed - st.link.lastAliveElapsedMs),
                i18n?.locale,
              ),
            })}
          {:else}
            {t("web.bar.idle")}
          {/if}
        </p>
        <div
          class="relative mt-3 h-3 overflow-hidden rounded-full bg-zinc-800"
          data-testid="parent_meter"
          role="img"
          aria-label={t("web.meter.a11y", { level: t(`web.meter.${levelWord}`) })}
        >
          <div
            class={`h-full transition-[width] ${
              st.alert != null ? "bg-red-500" : "bg-emerald-500"
            }`}
            style:width="{st.level * 100}%"
          ></div>
          {#if thresholdPct != null}
            <div
              class="absolute inset-y-0 w-0.5 bg-white/80"
              style:left="{thresholdPct}%"
              data-testid="parent_meter_threshold"
              aria-hidden="true"
            ></div>
          {/if}
        </div>
        <div class="mt-3 flex flex-wrap gap-2 text-xs">
          {#if st.peer.batteryPct != null}
            <span
              class={`rounded-full bg-zinc-800 px-3 py-1 ${stale ? "opacity-60" : ""}`}
              data-testid="parent_baby_battery_chip"
              data-stale={stale ? "true" : "false"}
            >
              {t("web.battery.baby", { percent: st.peer.batteryPct })}{atTime}
            </span>
          {/if}
          {#if st.peer.lowBattery}
            <span
              class="rounded-full bg-amber-900 px-3 py-1 text-amber-100"
              data-testid="parent_baby_battery_low"
            >
              {t("web.battery.babyLow", { percent: st.peer.lowBattery.pct })}
            </span>
          {/if}
          {#if st.peer.chargerUnplugged && !st.peer.unitWarnings.includes("not_charging")}
            <span class="rounded-full bg-amber-900 px-3 py-1 text-amber-100">
              {t("web.battery.babyNotCharging")}
            </span>
          {/if}
          <span
            class={`rounded-full px-3 py-1 ${stale ? "opacity-60" : ""} ${st.peer.silence === "unknown" ? "bg-zinc-800 text-zinc-300" : st.peer.silence === "silent" ? "bg-zinc-800" : "bg-amber-900 text-amber-100"}`}
            data-testid="parent_silence_chip"
            data-silence={st.peer.silence}
            data-stale={stale ? "true" : "false"}
          >
            {#if st.peer.silence === "silent"}
              {t("web.silence.silent")}
            {:else if st.peer.silence === "may_buzz"}
              {t("web.silence.mayVibrate")}
            {:else if st.peer.silence === "may_ring" || st.peer.silence === "no_access"}
              {t("web.silence.mayRing")}
            {:else}
              {t("web.silence.unknownManual")}
            {/if}{atTime}
          </span>
          {#if st.link.quality === "poor" && connected}
            <span
              class="rounded-full bg-amber-900 px-3 py-1 text-amber-100"
              data-testid="parent_quality_chip"
            >
              {t("web.quality.weak")}
            </span>
          {/if}
          {#each st.peer.unitWarnings as warning (warning)}
            {#if UNIT_WARNING_KEYS[warning]}
              <span
                class={`rounded-full bg-amber-900 px-3 py-1 text-amber-100 ${stale ? "opacity-60" : ""}`}
                data-testid="parent_unit_warning"
                data-warning={warning}
              >
                {t(UNIT_WARNING_KEYS[warning]!)}{atTime}
              </span>
            {/if}
          {/each}
          {#if snoozeActive}
            <span
              class="rounded-full bg-amber-900 px-3 py-1 text-amber-100"
              data-testid="parent_snooze_chip"
            >
              {t("web.snooze.active", { minutes: snoozeMinutesLeft })}
            </span>
          {/if}
        </div>
      </section>

      <ListenButton
        disabled={!connected}
        listening={listenInfo !== "off"}
        locked={listenInfo === "locked"}
        onListeningChange={setListening}
        onLockedChange={setListenLocked}
      />

      <div class="grid grid-cols-2 gap-2">
        <button
          type="button"
          class="min-h-12 rounded-lg border border-zinc-600 px-3 text-sm"
          data-testid="parent_snooze"
          onclick={() => (sheet = "snooze")}>{t("web.snooze.button")}</button
        >
        <button
          type="button"
          class="min-h-12 rounded-lg border border-zinc-600 px-3 text-sm"
          data-testid="parent_probe"
          disabled={!connected}
          onclick={() => (sheet = "probe")}>{t("web.probe.button")}</button
        >
      </div>

      {#if testQuestionAt != null && nowWall >= testQuestionAt}
        <div
          class="rounded-lg border border-zinc-700 bg-zinc-900 p-3"
          data-testid="sheet_probe_question"
        >
          <p>{t("web.probe.question")}</p>
          <div class="mt-2 flex gap-2">
            <button
              type="button"
              class="min-h-12 flex-1 rounded-lg bg-white/10"
              data-testid="sheet_probe_yes"
              onclick={() => answerTest("yes")}>{t("web.common.yes")}</button
            >
            <button
              type="button"
              class="min-h-12 flex-1 rounded-lg bg-white/10"
              data-testid="sheet_probe_no"
              onclick={() => answerTest("no")}>{t("web.common.no")}</button
            >
          </div>
        </div>
      {/if}
      {#if testAnswer === "no"}
        <p class="rounded-lg bg-amber-950 p-3 text-sm text-amber-100" data-testid="sheet_readiness">
          {t("web.probe.fixes")}
        </p>
      {:else if testAnswer === "yes"}
        <p class="text-sm text-emerald-300">{t("web.probe.ok")}</p>
      {/if}

      <button
        type="button"
        class="min-h-12 rounded-lg border border-red-900 px-4 text-sm text-red-200"
        data-testid="parent_stop"
        onclick={() => (sheet = "stop")}>{t("web.stop.button")}</button
      >
    {/if}
  {/if}
</div>

{#if sheet}
  <div
    class="fixed inset-0 z-40 flex items-end justify-center bg-black/70 p-4"
    role="dialog"
    aria-modal="true"
  >
    <div class="w-full max-w-md rounded-2xl border border-zinc-700 bg-zinc-900 p-5">
      {#if sheet === "snooze"}
        <h2 class="font-semibold" data-testid="sheet_snooze">{t("web.snooze.title")}</h2>
        <p class="mt-1 text-xs text-zinc-400">{t("web.snooze.help")}</p>
        <div class="mt-3 grid grid-cols-2 gap-2">
          <button
            type="button"
            class="min-h-12 rounded-lg bg-white/10"
            data-testid="sheet_snooze_10"
            onclick={() => {
              monitor?.snooze(10);
              sheet = null;
            }}>{t("web.snooze.minutes", { minutes: 10 })}</button
          >
          <button
            type="button"
            class="min-h-12 rounded-lg bg-white/10"
            data-testid="sheet_snooze_30"
            onclick={() => {
              monitor?.snooze(30);
              sheet = null;
            }}>{t("web.snooze.minutes", { minutes: 30 })}</button
          >
        </div>
        {#if snoozeActive}
          <button
            type="button"
            class="mt-2 min-h-12 w-full rounded-lg border border-zinc-600"
            data-testid="sheet_snooze_end"
            onclick={() => {
              monitor?.cancelSnooze();
              sheet = null;
            }}>{t("web.snooze.end")}</button
          >
        {/if}
      {:else if sheet === "probe"}
        <h2 class="font-semibold" data-testid="sheet_probe">{t("web.probe.title")}</h2>
        <p class="mt-1 text-sm text-zinc-300">{t("web.probe.body")}</p>
        <button
          type="button"
          class="mt-3 min-h-12 w-full rounded-lg bg-emerald-600 font-medium text-emerald-950"
          data-testid="sheet_probe_run"
          onclick={() => {
            monitor?.requestTestAlert();
            sheet = null;
          }}>{t("web.probe.run")}</button
        >
      {:else if sheet === "stop"}
        <h2 class="font-semibold">{t("web.stop.title")}</h2>
        <p class="mt-1 text-sm text-zinc-300">{t("web.stop.parentBody")}</p>
        <div class="mt-3 grid grid-cols-2 gap-2">
          <button
            type="button"
            class="min-h-12 rounded-lg bg-red-700"
            data-testid="dialog_stop_confirm"
            onclick={stopMonitoring}>{t("web.stop.confirm")}</button
          >
          <button
            type="button"
            class="min-h-12 rounded-lg bg-white/10"
            data-testid="dialog_stop_cancel"
            onclick={() => (sheet = null)}>{t("web.stop.cancel")}</button
          >
        </div>
      {/if}
      {#if sheet !== "stop"}
        <button
          type="button"
          class="mt-3 min-h-12 w-full rounded-lg border border-zinc-700 text-sm"
          onclick={() => (sheet = null)}>{t("web.common.close")}</button
        >
      {/if}
    </div>
  </div>
{/if}
