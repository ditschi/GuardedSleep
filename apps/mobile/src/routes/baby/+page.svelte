<script lang="ts">
  import { afterNavigate, goto } from "$app/navigation";
  import { browser } from "$app/environment";
  import { onDestroy, untrack } from "svelte";
  import { page } from "$app/state";
  import {
    startBabyMonitoringForeground,
    stopBabyMonitoringForeground,
  } from "$lib/android/foreground-monitoring.js";
  import { GuardedSleepNative, isNativeAndroid } from "$lib/android/guarded-sleep-native.js";
  import { subscribeNetworkHandoff } from "$lib/android/network-handoff.js";
  import { readBabyUnitPreferences } from "$lib/audio/baby-unit-preferences.js";
  import { NoiseMonitor } from "$lib/audio/noise-monitor.js";
  import ConnectionStatusBar from "$lib/components/ConnectionStatusBar.svelte";
  import { listConnections, upsertConnection } from "$lib/connection-store.js";
  import type { GuardedsleepDevTestApi } from "$lib/dev-e2e-bridge.js";
  import { buildCompactInviteJson } from "$lib/pairing/compact-invite.js";
  import {
    getStoredPairingPskB64ForRoom,
    takePairingPskForRoom,
  } from "$lib/pairing/session-material.js";
  import { resolveSignalingUrlForSession, validateSignalingUrl } from "$lib/signaling-url.js";
  import {
    SESSION_AUTO_RECONNECT_MAX_ATTEMPTS,
    sessionReconnectDelayMs,
  } from "$lib/webrtc/session-auto-reconnect.js";
  import { PeerSession } from "$lib/webrtc/peer-session.js";
  import { getI18n } from "$lib/i18n/context.js";
  import { connectionStatusPillI18nKey } from "$lib/i18n/session-state-pill.js";

  const i18n = getI18n();

  let status = $state<"idle" | "connecting" | "connected" | "failed">("idle");
  let session: PeerSession | undefined;
  let noiseMonitor = $state<NoiseMonitor | undefined>(undefined);
  let kioskImmersive = $state(true);

  let threshold = $state(readBabyUnitPreferences().threshold);
  let holdMs = $state(readBabyUnitPreferences().holdMs);
  let sessionReconnectAttempt = $state(0);
  let captureError = $state<string | undefined>(undefined);
  let prefsLoaded = $state(false);
  let reconnectEtaMs = $state(0);
  let reconnectTick = $state(0);
  let localRmsDisplay = $state(0);
  let prolongedWaitingForParent = $state(false);
  let startGeneration = 0;

  const room = $derived(page.url.searchParams.get("room") ?? "");
  const signalQ = $derived(page.url.searchParams.get("signal"));
  const signalingUrl = $derived(resolveSignalingUrlForSession(signalQ));

  const reconnectCountdownSec = $derived.by(() => {
    reconnectTick;
    if (reconnectEtaMs <= 0) return 0;
    return Math.max(0, Math.ceil((reconnectEtaMs - Date.now()) / 1000));
  });

  function applyPrefsFromStorage(): void {
    const p = readBabyUnitPreferences();
    threshold = p.threshold;
    holdMs = p.holdMs;
    kioskImmersive = p.kioskImmersive;
  }

  $effect(() => {
    if (!browser || prefsLoaded) return;
    applyPrefsFromStorage();
    prefsLoaded = true;
  });

  $effect(() => {
    if (!browser || !prefsLoaded || !room) return;
    const u = signalingUrl;
    if (validateSignalingUrl(u) !== null) return;
    const t = window.setTimeout(() => start(), 0);
    return () => clearTimeout(t);
  });

  $effect(() => {
    const t = threshold;
    const h = holdMs;
    noiseMonitor?.setGate(t, h);
  });

  $effect(() => {
    if (!browser) return;
    const onVis = (): void => {
      if (document.visibilityState === "visible") noiseMonitor?.resumeAudioContext();
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  });

  $effect(() => {
    if (!browser || !isNativeAndroid() || !room) return;
    const on = kioskImmersive;
    void GuardedSleepNative.setImmersiveMode({ enabled: on });
  });

  $effect(() => {
    if (!browser) return;
    const r = room;
    const st = status;
    if (!r) return;
    if (st === "connected") {
      sessionReconnectAttempt = 0;
      reconnectEtaMs = 0;
      return;
    }
    if (st !== "failed") return;
    const prev = untrack(() => sessionReconnectAttempt);
    if (prev >= SESSION_AUTO_RECONNECT_MAX_ATTEMPTS) {
      reconnectEtaMs = 0;
      return;
    }
    const next = prev + 1;
    sessionReconnectAttempt = next;
    const ms = sessionReconnectDelayMs(next);
    reconnectEtaMs = Date.now() + ms;
    const t = window.setTimeout(() => start(), ms);
    return () => clearTimeout(t);
  });

  $effect(() => {
    if (!browser || status !== "failed" || reconnectEtaMs <= 0) return;
    const id = window.setInterval(() => {
      reconnectTick += 1;
    }, 400);
    return () => clearInterval(id);
  });

  $effect(() => {
    if (!browser || !noiseMonitor) return;
    const id = window.setInterval(() => {
      localRmsDisplay = noiseMonitor?.getLastRms() ?? 0;
    }, 150);
    return () => clearInterval(id);
  });

  $effect(() => {
    if (!browser || !room) {
      prolongedWaitingForParent = false;
      return;
    }
    if (status === "connecting" || status === "connected") {
      prolongedWaitingForParent = false;
      return;
    }
    const id = window.setTimeout(() => {
      prolongedWaitingForParent = true;
    }, 60_000);
    return () => clearTimeout(id);
  });

  $effect(() => {
    if (!browser || !isNativeAndroid()) return;
    return subscribeNetworkHandoff(() => start({ resetBackoff: true }));
  });

  let batteryLevel = $state<number | undefined>(undefined);

  $effect(() => {
    let cancelled = false;
    type Bat = {
      readonly level: number;
      addEventListener(type: "levelchange", fn: () => void): void;
      removeEventListener(type: "levelchange", fn: () => void): void;
    };
    let bat: Bat | undefined;
    const onLvl = (): void => {
      if (bat) batteryLevel = bat.level;
    };
    void (async () => {
      try {
        const gb = (navigator as Navigator & { getBattery?: () => Promise<Bat> }).getBattery;
        const b = await gb?.();
        if (!b || cancelled) return;
        bat = b;
        batteryLevel = b.level;
        b.addEventListener("levelchange", onLvl);
      } catch {
        /* no Battery API */
      }
    })();
    return () => {
      cancelled = true;
      bat?.removeEventListener("levelchange", onLvl);
    };
  });

  function start(opts?: { resetBackoff?: boolean }): void {
    void startAsync(opts);
  }

  async function startAsync(opts?: { resetBackoff?: boolean }): Promise<void> {
    const gen = ++startGeneration;
    if (opts?.resetBackoff) {
      sessionReconnectAttempt = 0;
      reconnectEtaMs = 0;
    }
    captureError = undefined;
    noiseMonitor?.stop();
    session?.close();
    applyPrefsFromStorage();
    const psk = takePairingPskForRoom(room);
    const pairing = psk != null && psk.length >= 16 ? { roomId: room, pskKey: psk } : undefined;

    let prependSignalingUrls: string[] | undefined;
    if (browser && isNativeAndroid() && room) {
      try {
        await GuardedSleepNative.stopLanSignalingServer();
      } catch {
        /* ignore */
      }
      try {
        const { port } = await GuardedSleepNative.startLanSignalingServer({ roomId: room });
        if (gen !== startGeneration) return;
        const discoveryToken = crypto.randomUUID().replace(/-/g, "").slice(0, 16);
        await GuardedSleepNative.startNsdAdvertise({ port, discoveryToken });
        if (gen !== startGeneration) return;
        prependSignalingUrls = [`ws://127.0.0.1:${port}`];
      } catch {
        prependSignalingUrls = undefined;
      }
      void GuardedSleepNative.setKeepAwake({ enabled: true });
      void GuardedSleepNative.setImmersiveMode({ enabled: kioskImmersive });
    }
    if (gen !== startGeneration) return;

    void startBabyMonitoringForeground();

    const savedFallbacks = listConnections().find(
      (c) => c.roomId === room && c.signalingUrl === signalingUrl,
    )?.fallbackSignalingUrls;

    const s = new PeerSession(
      "baby",
      room,
      signalingUrl,
      {
        onState: (st) => {
          status = st;
          if (st === "connected") {
            upsertConnection({ roomId: room, signalingUrl, label: "Baby" });
          }
        },
        onLocalMediaError: (msg) => {
          captureError = msg;
        },
        onLocalCapture: (stream) => {
          noiseMonitor?.stop();
          noiseMonitor = new NoiseMonitor(stream, {
            threshold,
            holdMs,
            onLoud: (rms) => {
              s.setNoiseAlertStreaming(true);
              s.sendNoiseAlert(rms);
            },
            onQuiet: () => {
              s.setNoiseAlertStreaming(false);
            },
          });
          noiseMonitor.start();
        },
        onOpenSettingsRequest: () => {
          if (!browser || !room) return;
          const q = new URLSearchParams({ returnTo: "baby", room });
          if (signalQ) q.set("signal", signalQ);
          void goto(`/settings?${q}`);
        },
      },
      {
        pairing,
        getHeartbeatExtras: () => ({
          battery: batteryLevel,
          rms: noiseMonitor?.getLastRms(),
        }),
        ...(prependSignalingUrls && prependSignalingUrls.length > 0
          ? { prependSignalingUrls }
          : {}),
        ...(savedFallbacks && savedFallbacks.length > 0
          ? { fallbackSignalingUrls: savedFallbacks }
          : {}),
      },
    );
    if (gen !== startGeneration) {
      s.close();
      return;
    }
    session = s;
    s.start();
  }

  async function openBatterySettings(): Promise<void> {
    if (!browser) return;
    if (isNativeAndroid()) {
      await GuardedSleepNative.openBatterySettings();
      return;
    }
    window.alert(i18n?.t("baby.batteryAlertWeb") ?? "");
  }

  function openPairingQr(): void {
    if (!browser || !room) return;
    const q = new URLSearchParams({ role: "baby", room, pairHost: "1" });
    if (signalQ) q.set("signal", signalQ);
    void goto(`/pair?${q}`);
  }

  afterNavigate(() => {
    if (page.url.pathname !== "/baby") return;
    applyPrefsFromStorage();
  });

  $effect(() => {
    if (!import.meta.env.DEV || !browser) return;
    const api: GuardedsleepDevTestApi = {
      role: "baby",
      getSession: () => session,
      getInviteJson: () => {
        const k = getStoredPairingPskB64ForRoom(room) ?? "";
        if (!room || !signalingUrl || !k) return "";
        return buildCompactInviteJson(room, signalingUrl, k);
      },
      setNoiseGate: (t, h) => {
        threshold = t;
        holdMs = h;
      },
    };
    window.__guardedsleep_test = api;
    return () => {
      if (window.__guardedsleep_test === api) delete window.__guardedsleep_test;
    };
  });

  onDestroy(() => {
    noiseMonitor?.stop();
    session?.close();
    void stopBabyMonitoringForeground();
    if (browser && isNativeAndroid()) {
      void GuardedSleepNative.stopLanSignalingServer();
      void GuardedSleepNative.stopNsdAdvertise();
      void GuardedSleepNative.setKeepAwake({ enabled: false });
      void GuardedSleepNative.setImmersiveMode({ enabled: false });
    }
  });
</script>

<div class="flex min-h-[100dvh] flex-col gap-4 bg-black px-4 py-6 text-zinc-100">
  <ConnectionStatusBar
    role="baby"
    {status}
    {reconnectCountdownSec}
    reconnectAttempt={sessionReconnectAttempt}
    maxAttempts={SESSION_AUTO_RECONNECT_MAX_ATTEMPTS}
  />
  <header class="flex flex-wrap items-center justify-between gap-2">
    <h1 class="text-lg font-semibold text-emerald-400">{i18n?.t("baby.title") ?? ""}</h1>
    <div class="flex flex-wrap items-center gap-2">
      {#if status === "connected"}
        <span
          class="rounded-full bg-emerald-950/80 px-2.5 py-1 text-xs font-medium text-emerald-200 ring-1 ring-emerald-700/50"
          data-testid="parent-link-pill"
          title={i18n?.t("baby.parentConnectedTitle") ?? ""}
        >
          {i18n?.t("baby.parentConnected") ?? ""}
        </span>
      {:else}
        <span
          class={`rounded-full bg-zinc-900 px-2.5 py-1 text-xs font-medium ring-1 ${prolongedWaitingForParent ? "guardedsleep-slow-pulse ring-red-500/60 text-red-100" : "text-amber-200/90 ring-zinc-700"}`}
          data-testid="parent-link-pill"
          title={i18n?.t("baby.waitingParentTitle") ?? ""}
        >
          {i18n?.t("baby.waitingParent") ?? ""}
        </span>
      {/if}
      <span
        class="rounded-full bg-zinc-900 px-2 py-1 text-xs text-zinc-400"
        data-testid="connection-status"
        data-session-state={status}>{i18n?.t(connectionStatusPillI18nKey(status)) ?? status}</span
      >
    </div>
  </header>
  <p class="text-sm text-zinc-500">
    {i18n?.t("baby.roomLabel") ?? ""} <span class="font-mono text-zinc-300">{room || "—"}</span>
  </p>
  {#if room}
    <p class="text-xs leading-relaxed text-zinc-500">
      {#if status === "connected"}
        {i18n?.t("baby.blurbConnected") ?? ""}
      {:else if status === "connecting"}
        {i18n?.t("baby.blurbConnecting") ?? ""}
      {:else if status === "failed"}
        {i18n?.t("baby.blurbFailed") ?? ""}
      {:else}
        {i18n?.t("baby.blurbIdle") ?? ""}
      {/if}
    </p>
  {/if}
  {#if !room}
    <p class="text-amber-300">
      {i18n?.t("baby.missingRoom") ?? ""}
      <a class="text-emerald-400 underline" href="/pair?role=baby"
        >{i18n?.t("baby.configurePairing") ?? ""}</a
      >
    </p>
  {:else}
    {#if captureError}
      <p
        class="rounded-lg border border-amber-800/80 bg-amber-950/30 px-3 py-2 text-sm text-amber-100/95"
        role="alert"
      >
        {captureError}
      </p>
    {/if}

    {#if status === "failed"}
      <div
        class="rounded-lg border border-amber-800/80 bg-amber-950/25 px-3 py-3 text-sm text-amber-100/95"
        role="status"
      >
        <p class="font-medium text-amber-100">{i18n?.t("baby.failedTitle") ?? ""}</p>
        <p class="mt-1 text-xs text-amber-200/80">
          {i18n?.t("baby.failedHint") ?? ""}
        </p>
        {#if sessionReconnectAttempt > 0 && sessionReconnectAttempt <= SESSION_AUTO_RECONNECT_MAX_ATTEMPTS}
          <p class="mt-2 text-xs text-amber-200/90">
            {i18n?.t("baby.retryLine", {
              seconds: reconnectCountdownSec,
              attempt: sessionReconnectAttempt,
              max: SESSION_AUTO_RECONNECT_MAX_ATTEMPTS,
            }) ?? ""}
          </p>
        {:else if sessionReconnectAttempt > SESSION_AUTO_RECONNECT_MAX_ATTEMPTS}
          <p class="mt-2 text-xs text-amber-200/90">
            {i18n?.t("baby.retriesStopped") ?? ""}
          </p>
        {/if}
        <button
          type="button"
          class="mt-3 w-full rounded-lg bg-amber-600 px-3 py-2 text-sm font-medium text-amber-950 hover:bg-amber-500"
          onclick={() => start({ resetBackoff: true })}
        >
          {i18n?.t("baby.tryNow") ?? ""}
        </button>
      </div>
    {/if}

    <div class="rounded-lg border border-zinc-800 bg-zinc-950/80 p-3">
      <p class="mb-1 text-xs uppercase tracking-wide text-zinc-500">
        {i18n?.t("baby.micLevelTitle") ?? ""}
      </p>
      <div class="h-3 overflow-hidden rounded-full bg-zinc-800">
        <div
          class="h-full bg-emerald-500 transition-[width]"
          style:width="{Math.min(100, localRmsDisplay * 400)}%"
        ></div>
      </div>
      <p class="mt-1 text-[11px] text-zinc-600">
        {i18n?.t("baby.micLevelHelp") ?? ""}
      </p>
    </div>

    <div class="flex flex-wrap gap-2">
      <button
        type="button"
        class="rounded-lg bg-emerald-700 px-4 py-2 text-sm font-medium text-white hover:bg-emerald-600"
        data-testid="session-start"
        onclick={() => start({ resetBackoff: true })}
      >
        {i18n?.t("baby.restartMonitoring") ?? ""}
      </button>
      <button
        type="button"
        class="rounded-lg border border-zinc-600 px-4 py-2 text-sm text-zinc-300 hover:bg-zinc-900"
        onclick={openPairingQr}
      >
        {i18n?.t("baby.showPairingQr") ?? ""}
      </button>
      {#if isNativeAndroid()}
        <button
          type="button"
          class="rounded-lg border border-zinc-600 px-4 py-2 text-sm text-zinc-300 hover:bg-zinc-900"
          onclick={openBatterySettings}
        >
          {i18n?.t("baby.batterySettings") ?? ""}
        </button>
      {/if}
      <a
        class="inline-flex items-center rounded-lg border border-zinc-600 px-4 py-2 text-sm text-zinc-300 hover:bg-zinc-900"
        href={signalQ
          ? `/settings?returnTo=baby&room=${encodeURIComponent(room)}&signal=${encodeURIComponent(signalQ)}`
          : `/settings?returnTo=baby&room=${encodeURIComponent(room)}`}
      >
        {i18n?.t("baby.soundKioskSettings") ?? ""}
      </a>
    </div>
    <p class="text-xs leading-relaxed text-zinc-600">
      {i18n?.t("baby.footer") ?? ""}
    </p>
  {/if}
</div>
