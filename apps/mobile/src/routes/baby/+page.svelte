<script lang="ts">
  import { browser } from "$app/environment";
  import { goto } from "$app/navigation";
  import { onDestroy, onMount } from "svelte";
  import { readBabyUnitPreferences } from "$lib/audio/baby-unit-preferences.js";
  import {
    babyMicrophoneBlockReason,
    formatBabyMicrophoneError,
    requestBabyMicrophone,
    takePrewarmedBabyMicrophone,
  } from "$lib/audio/baby-microphone.js";
  import { createDevSyntheticMicStream } from "$lib/audio/dev-synthetic-mic.js";
  import AmbientBar from "$lib/components/AmbientBar.svelte";
  import KioskLockOverlay from "$lib/components/KioskLockOverlay.svelte";
  import PairingQr from "$lib/components/PairingQr.svelte";
  import type { GuardedsleepDevTestApi } from "$lib/dev-e2e-bridge.js";
  import { getI18n } from "$lib/i18n/context.js";
  import { BabyMonitor } from "$lib/link/baby-monitor.js";
  import { getCore } from "$lib/link/core-loader.js";
  import { formatDuration } from "$lib/link/format.js";
  import { buildLinkConfig } from "$lib/link/link-config.js";
  import { initialBabyState, type BabyMonitorState } from "$lib/link/ui-contract.js";
  import { createWakeLock, trackVisibility, watchCharging } from "$lib/link/web-readiness.js";
  import {
    clearActivePairing,
    getActivePairing,
    markPairedBefore,
    setActivePairing,
  } from "$lib/pairing/pairing-memory.js";
  import { generatePairingCode } from "$lib/pairing/pairing-v3.js";
  import { getEffectiveSignalingUrl } from "$lib/signaling-url.js";

  const i18n = getI18n();
  const t = (key: string, params?: Record<string, string | number>): string =>
    i18n?.t(key, params) ?? key;

  let monitor: BabyMonitor | undefined = $state();
  let st: BabyMonitorState = $state(initialBabyState());
  let micStream: MediaStream | null = $state(null);
  let micError: string | null = $state(null);
  let running = $state(false);
  let nowElapsed = $state(0);
  let startedElapsed = $state(0);
  let nowWall = $state(Date.now());
  let paired = $state(getActivePairing()?.pairedBefore ?? false);
  let codePayload = $state(getActivePairing()?.codePayload ?? "");
  let codeExpiresAt = $state(getActivePairing()?.codeExpiresAtWallMs ?? 0);
  let bright = $state(false);
  let copied = $state(false);
  let textShown = $state(false);
  let confirmPairAnother = $state(false);
  let hiddenNow = $state(false);
  let suspendWarning = $state(false);
  let confirmed = $state({ lid: false, sleep: false });
  let showStop = $state(false);
  let locked = $state(false);

  const wake = createWakeLock();
  const wakeActive = wake.active;
  const charging = browser ? watchCharging() : null;
  const visibility = browser ? trackVisibility() : null;
  const unsubs: (() => void)[] = [];

  const expired = $derived(codeExpiresAt > 0 && nowWall >= codeExpiresAt);
  const remainingMs = $derived(Math.max(0, codeExpiresAt - nowWall));
  const showCode = $derived(running && !paired && codePayload !== "");
  const notCharging = $derived(charging ? $charging === false : false);
  const kioskState = $derived(
    st.link.phase === "connected" ? "connected" : st.link.level >= 2 ? "failed" : "connecting",
  );

  onMount(() => {
    const clock = setInterval(() => {
      nowWall = Date.now();
      if (monitor) nowElapsed = monitor.link.now();
    }, 1000);
    const offVis = visibility?.state.subscribe((v) => (hiddenNow = v.hidden));
    void ensureMic();
    return () => {
      clearInterval(clock);
      offVis?.();
    };
  });

  async function ensureMic(): Promise<void> {
    if (micStream) return;
    const params = new URLSearchParams(location.search);
    try {
      if (import.meta.env.DEV && params.get("e2e_synthetic_audio") === "1") {
        micStream = await createDevSyntheticMicStream();
      } else {
        micStream = takePrewarmedBabyMicrophone() ?? (await requestBabyMicrophone());
      }
      micError = null;
    } catch (e) {
      micError = formatBabyMicrophoneError(e);
    }
  }

  function freshPairing(): void {
    const urls = [getEffectiveSignalingUrl()];
    const code = generatePairingCode(urls, Date.now());
    setActivePairing({
      ...code.material,
      pairedBefore: false,
      codePayload: code.payload,
      codeExpiresAtWallMs: code.expiresAtWallMs,
    });
    codePayload = code.payload;
    codeExpiresAt = code.expiresAtWallMs;
    paired = false;
  }

  async function startRun(): Promise<void> {
    if (running) return;
    await ensureMic();
    if (!micStream) return;
    if (!getActivePairing()) freshPairing();
    const pairing = getActivePairing()!;
    const core = await getCore();
    const unit = readBabyUnitPreferences();
    const m = new BabyMonitor({
      core,
      config: buildLinkConfig("baby", pairing.pskB64Url, pairing.pairedBefore, {
        threshold: unit.threshold,
        holdMs: unit.holdMs,
      }),
      signalingUrls: pairing.signalingUrls,
      stream: micStream,
      onVerified: () => {
        markPairedBefore();
        paired = true;
      },
    });
    micStream = null;
    if (
      import.meta.env.DEV &&
      new URLSearchParams(location.search).get("e2e_synthetic_levels") === "1"
    ) {
      m.babyAudio.syntheticOnly = true;
    }
    unsubs.push(m.state.subscribe((s) => (st = s)));
    unsubs.push(
      m.suspendGap.subscribe((g) => {
        if (g) suspendWarning = true;
      }),
    );
    monitor = m;
    startedElapsed = m.link.now();
    nowElapsed = startedElapsed;
    m.start();
    running = true;
    wake.start();
  }

  function endRun(): void {
    for (const u of unsubs.splice(0)) u();
    monitor?.stop();
    monitor = undefined;
    running = false;
    wake.stop();
  }

  async function newCode(): Promise<void> {
    confirmPairAnother = false;
    textShown = false;
    endRun();
    freshPairing();
    await startRun();
  }

  async function stopMonitoring(): Promise<void> {
    endRun();
    clearActivePairing();
    showStop = false;
    await goto("/");
  }

  async function copyCode(): Promise<void> {
    try {
      await navigator.clipboard.writeText(codePayload);
      copied = true;
    } catch {
      copied = false;
    }
  }

  async function lockScreen(): Promise<void> {
    locked = true;
    monitor?.setKioskLocked(true);
    try {
      await document.documentElement.requestFullscreen?.();
    } catch {
      /* fullscreen is optional */
    }
  }

  async function unlockScreen(): Promise<void> {
    locked = false;
    monitor?.setKioskLocked(false);
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
    } catch {
      /* ignore */
    }
  }

  $effect(() => {
    monitor?.setPageSignals({ hidden: hiddenNow, wakeLockHeld: $wakeActive });
  });

  $effect(() => {
    if (!import.meta.env.DEV || !browser) return;
    const api: GuardedsleepDevTestApi = {
      role: "baby",
      getSession: () =>
        monitor
          ? {
              getE2ePeerConnection: () => monitor!.link.currentPeer()?.pc ?? null,
              getE2eDataChannel: () => monitor!.link.currentPeer()?.dataChannel() ?? null,
              getE2ePairingVerified: () => monitor!.link.status?.state === "connected",
              getE2eBabyLocalAudioTrackEnabled: () => monitor!.babyAudio.outgoingEnabled(),
              sendListen: () => {},
            }
          : undefined,
      getInviteJson: () => getActivePairing()?.codePayload ?? "",
      injectAudioLevel: (rms, durationMs) => monitor?.babyAudio.inject(rms, durationMs),
      setKioskLocked: (v) => (v ? void lockScreen() : void unlockScreen()),
      isKioskLocked: () => locked,
      getLinkSnapshot: () => ({ state: st, nowElapsed: monitor?.link.now() ?? 0 }),
    };
    window.__guardedsleep_test = api;
    return () => {
      if (window.__guardedsleep_test === api) delete window.__guardedsleep_test;
    };
  });

  onDestroy(() => {
    endRun();
    visibility?.destroy();
  });
</script>

<div class="min-h-[100dvh] bg-black px-4 py-6 text-zinc-400">
  <div class="mx-auto flex max-w-md flex-col gap-4">
    <h1 class="text-lg font-medium text-zinc-400">{t("web.baby.title")}</h1>

    {#if !running}
      <section class="rounded-xl border border-zinc-900 p-4" data-testid="baby_setup">
        <h2 class="font-medium text-zinc-300" data-testid="setup_title">{t("web.setup.title")}</h2>
        <ul class="mt-3 space-y-3 text-sm">
          <li
            data-testid="setup_row_mic"
            data-state={micStream ? "done" : micError ? "blocked" : "checking"}
          >
            <span aria-hidden="true">{micStream ? "✓" : micError ? "✕" : "…"}</span>
            {t("web.setup.mic")}
            {#if micError}
              <span class="block text-xs text-red-300">{micError}</span>
              {#if babyMicrophoneBlockReason() === "insecure"}
                <span class="block text-xs">{t("web.setup.micInsecure")}</span>
              {/if}
              <button
                type="button"
                class="mt-2 min-h-12 rounded-lg border border-zinc-700 px-4 text-zinc-300"
                data-testid="setup_mic_allow"
                onclick={() => void ensureMic()}>{t("web.setup.micAllow")}</button
              >
            {/if}
          </li>
          <li data-testid="setup_row_power" data-state={$charging === true ? "done" : "todo"}>
            <span aria-hidden="true">{$charging === true ? "✓" : "!"}</span>
            {t("web.setup.power")}
            {#if $charging == null}
              <span class="block text-xs">{t("web.setup.powerUnknown")}</span>
            {:else if $charging === false}
              <span class="block text-xs text-amber-400">{t("web.setup.powerOnBattery")}</span>
            {/if}
          </li>
          <li>
            <label class="flex items-start gap-2">
              <input
                type="checkbox"
                bind:checked={confirmed.lid}
                class="mt-1 h-5 w-5"
                data-testid="setup_row_lid"
              />
              <span>{t("web.setup.lid")}</span>
            </label>
          </li>
          <li>
            <label class="flex items-start gap-2">
              <input
                type="checkbox"
                bind:checked={confirmed.sleep}
                class="mt-1 h-5 w-5"
                data-testid="setup_row_sleep"
              />
              <span>{t("web.setup.sleep")}</span>
            </label>
          </li>
          <li data-testid="setup_row_visible">
            <span aria-hidden="true">i</span>
            {t("web.setup.visible")}
          </li>
          <li data-testid="setup_row_reload">
            <span aria-hidden="true">i</span>
            {t("web.setup.reloadNote")}
          </li>
          {#if !wake.supported}
            <li class="text-amber-400">{t("web.keepOpen.noWakeLock")}</li>
          {/if}
        </ul>
        <button
          type="button"
          class="mt-4 min-h-14 w-full rounded-xl border border-zinc-700 bg-zinc-950 px-4 font-medium text-zinc-300 disabled:opacity-40"
          data-testid="session-start"
          disabled={!micStream}
          onclick={() => void startRun()}
        >
          {t("web.setup.start")}
        </button>
      </section>
    {:else}
      {#if notCharging}
        <p
          class="rounded-lg border border-amber-950 p-3 text-sm text-amber-600"
          role="alert"
          data-testid="baby_warning_battery"
        >
          {t("web.warn.notCharging")}
        </p>
      {/if}
      {#if hiddenNow}
        <p
          class="rounded-lg border border-amber-950 p-3 text-sm text-amber-600"
          role="alert"
          data-testid="baby_warning_hidden"
        >
          {t("web.warn.hidden")}
        </p>
      {/if}
      {#if suspendWarning}
        <div
          class="rounded-lg border border-amber-950 p-3 text-sm text-amber-600"
          role="alert"
          data-testid="baby_warning_suspend"
        >
          <p>{t("web.suspend.warningBaby")}</p>
          <button
            type="button"
            class="mt-2 min-h-12 rounded-lg border border-zinc-800 px-4"
            onclick={() => (suspendWarning = false)}>{t("web.common.ok")}</button
          >
        </div>
      {/if}
      {#if !$wakeActive}
        <p class="text-xs text-amber-500" data-testid="baby_warning_wakelock">
          {wake.supported ? t("web.keepOpen.wakeLockOff") : t("web.keepOpen.noWakeLock")}
        </p>
      {/if}

      <AmbientBar
        link={st.link}
        role="baby"
        testid="baby_status"
        dim
        nowElapsedMs={nowElapsed}
        startedElapsedMs={startedElapsed}
        onTryNow={() => monitor?.tryNow()}
      />

      <ul class="space-y-1 text-xs" data-testid="baby_reminders">
        {#if $charging === true}
          <li data-testid="baby_reminder_power">
            <span aria-hidden="true">✓</span>
            {t("web.setup.power")}
          </li>
        {:else if $charging == null}
          <li data-testid="baby_reminder_power">
            <span aria-hidden="true">!</span>
            {t("web.setup.powerUnknown")}
          </li>
        {/if}
        <li data-testid="baby_reminder_visible">
          <span aria-hidden="true">i</span>
          {t("web.setup.visible")}
        </li>
      </ul>

      {#if showCode}
        <section class="rounded-xl border border-zinc-900 p-4 text-center" data-testid="baby_pair">
          <h2 class="font-medium text-zinc-300" data-testid="baby_pair_title">
            {t("web.pair.baby.title")}
          </h2>
          <p class="mt-1 text-xs" data-testid="baby_pair_instruction">
            {t("web.pair.baby.instruction")}
          </p>
          <div class="mt-3">
            <PairingQr payload={codePayload} alt={t("web.pair.baby.qrAlt")} {expired} {bright} />
          </div>
          {#if expired}
            <p class="mt-2 text-sm text-amber-600" data-testid="baby_pair_expired">
              {t("web.pair.baby.expired")}
            </p>
            <button
              type="button"
              class="mt-2 min-h-12 rounded-lg border border-zinc-700 px-4 text-zinc-300"
              data-testid="baby_pair_new_code"
              onclick={() => void newCode()}>{t("web.pair.baby.newCode")}</button
            >
          {:else}
            <p class="mt-2 text-xs" data-testid="baby_pair_countdown">
              {t("web.pair.baby.validFor", { duration: formatDuration(remainingMs) })}
            </p>
            <p class="mt-1 text-xs" data-testid="baby_pair_waiting">{t("web.pair.baby.waiting")}</p>
            <div class="mt-3 flex flex-wrap justify-center gap-2">
              <button
                type="button"
                class="min-h-12 rounded-lg border border-zinc-700 px-3 text-zinc-300"
                data-testid="baby_pair_copy"
                onclick={() => void copyCode()}
                >{copied ? t("web.pair.baby.copied") : t("web.pair.baby.copy")}</button
              >
              <button
                type="button"
                class="min-h-12 rounded-lg border border-zinc-700 px-3 text-zinc-300"
                data-testid="baby_pair_brighter"
                onclick={() => (bright = !bright)}>{t("web.pair.baby.brighter")}</button
              >
              <button
                type="button"
                class="min-h-12 rounded-lg border border-zinc-700 px-3 text-zinc-300"
                data-testid="baby_pair_show_text"
                aria-expanded={textShown}
                onclick={() => (textShown = !textShown)}
                >{textShown ? t("web.pair.baby.hideText") : t("web.pair.baby.showText")}</button
              >
            </div>
            {#if textShown}
              <textarea
                readonly
                class="mt-3 h-20 w-full rounded border border-zinc-700 bg-black p-2 text-[10px] text-zinc-300"
                data-testid="baby_pair_code_text"
                value={codePayload}
              ></textarea>
            {/if}
          {/if}
          <p class="mt-2 text-xs text-zinc-400" data-testid="baby_pair_privacy">
            {t("web.pair.baby.privacy")}
          </p>
        </section>
      {:else}
        <section class="rounded-xl border border-zinc-900 p-4 text-sm">
          <p data-testid="baby_monitoring_line">{t("web.baby.monitoring")}</p>
          <div
            class="mt-3 h-1.5 overflow-hidden rounded bg-zinc-900"
            data-testid="baby_mic_level"
            role="img"
            aria-label={t("web.baby.micLevel")}
          >
            <div class="h-full bg-zinc-700" style:width="{Math.min(100, st.micLevel * 400)}%"></div>
          </div>
          {#if st.micSilent}
            <p class="mt-2 text-xs text-amber-500">{t("web.baby.micSilent")}</p>
          {/if}
          <p class="mt-3 text-xs" data-testid="baby_silence">{t("web.baby.silenceManual")}</p>
          {#if paired && !confirmPairAnother}
            <button
              type="button"
              class="mt-3 min-h-12 rounded-lg border border-zinc-700 px-3 text-zinc-300"
              data-testid="baby_pair_another"
              onclick={() => (confirmPairAnother = true)}>{t("web.pair.baby.pairAnother")}</button
            >
          {:else if paired}
            <div
              class="mt-3 rounded-lg border border-amber-900 p-3"
              role="alertdialog"
              data-testid="baby_pair_another_confirm_box"
            >
              <p class="text-amber-400">{t("web.pair.baby.pairAnotherConfirm")}</p>
              <div class="mt-2 grid grid-cols-2 gap-2">
                <button
                  type="button"
                  class="min-h-12 rounded-lg border border-zinc-700 px-3 text-zinc-300"
                  data-testid="baby_pair_another_confirm"
                  onclick={() => void newCode()}>{t("web.pair.baby.pairAnother")}</button
                >
                <button
                  type="button"
                  class="min-h-12 rounded-lg border border-zinc-700 px-3 text-zinc-300"
                  data-testid="baby_pair_another_cancel"
                  onclick={() => (confirmPairAnother = false)}>{t("web.common.cancel")}</button
                >
              </div>
            </div>
          {/if}
        </section>
      {/if}

      <div class="grid grid-cols-2 gap-2">
        <button
          type="button"
          class="min-h-14 rounded-xl border border-zinc-800 px-4 text-zinc-300"
          data-testid="baby_lock"
          onclick={() => void lockScreen()}>{t("web.baby.lock")}</button
        >
        <button
          type="button"
          class="min-h-14 rounded-xl border border-zinc-800 px-4 text-zinc-400"
          data-testid="baby_stop"
          onclick={() => (showStop = true)}>{t("web.stop.button")}</button
        >
      </div>
    {/if}
  </div>
</div>

{#if showStop}
  <div
    class="fixed inset-0 z-40 flex items-end justify-center bg-black/90 p-4"
    role="dialog"
    aria-modal="true"
  >
    <div class="w-full max-w-md rounded-2xl border border-zinc-800 bg-black p-5 text-zinc-400">
      <h2 class="font-medium text-zinc-300">{t("web.stop.title")}</h2>
      <p class="mt-1 text-sm">{t("web.stop.babyBody")}</p>
      <div class="mt-3 grid grid-cols-2 gap-2">
        <button
          type="button"
          class="min-h-12 rounded-lg border border-red-950 text-red-400"
          data-testid="dialog_stop_confirm"
          onclick={() => void stopMonitoring()}>{t("web.stop.confirm")}</button
        >
        <button
          type="button"
          class="min-h-12 rounded-lg border border-zinc-800"
          data-testid="dialog_stop_cancel"
          onclick={() => (showStop = false)}>{t("web.stop.cancel")}</button
        >
      </div>
    </div>
  </div>
{/if}

{#if locked}
  <div
    class="fixed inset-0 z-50 bg-black"
    data-testid="baby_kiosk"
    aria-label={t("web.kiosk.a11yLabel")}
  >
    <KioskLockOverlay connectionStatus={kioskState} onUnlock={() => void unlockScreen()} />
  </div>
{/if}
