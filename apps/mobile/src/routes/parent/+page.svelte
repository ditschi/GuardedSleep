<script lang="ts">
  import { browser } from "$app/environment";
  import { onDestroy, untrack } from "svelte";
  import { page } from "$app/state";
  import {
    ensureParentMonitorNotificationsReady,
    notifyBabyLinkUncertain,
    notifyNoiseAlertIfBackground,
  } from "$lib/android/parent-monitor-notifications.js";
  import { GuardedSleepNative, isNativeAndroid } from "$lib/android/guarded-sleep-native.js";
  import type { LanPeerEntry } from "$lib/android/lan-peer-signaling-urls.js";
  import { lanPrependSignalingUrls } from "$lib/android/lan-peer-signaling-urls.js";
  import { subscribeNetworkHandoff } from "$lib/android/network-handoff.js";
  import ConnectionStatusBar from "$lib/components/ConnectionStatusBar.svelte";
  import { listConnections, upsertConnection } from "$lib/connection-store.js";
  import type { GuardedsleepDevTestApi } from "$lib/dev-e2e-bridge.js";
  import {
    persistLastSnoozeMinutes,
    readLastSnoozeMinutes,
  } from "$lib/parent-snooze-preferences.js";
  import { takePairingPskForRoom } from "$lib/pairing/session-material.js";
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
  let audioEl: HTMLAudioElement | null = $state(null);
  let listenOn = $state(false);
  let quality = $state<"good" | "medium" | "poor" | "unknown">("unknown");
  let babyBattery = $state<number | undefined>(undefined);
  let lastRemoteRms = $state<number | undefined>(undefined);
  let quietSince = $state<Date | undefined>(undefined);
  let snoozeUntil = $state(0);
  let remoteStream = $state<MediaStream | null>(null);
  let lanPeerMap = $state<Record<string, LanPeerEntry>>({});
  let lanListener: { remove: () => Promise<void> } | undefined;
  let sessionReconnectAttempt = $state(0);
  let reconnectEtaMs = $state(0);
  let reconnectTick = $state(0);

  let lastBabyHeartbeatAt = 0;
  let babyLinkAlertSent = false;
  let devNoiseAlertResolver: ((rms: number) => void) | null = null;
  let devNoiseAlertCount = 0;
  let pendingListenResume = false;

  let snoozeDialogEl: HTMLDialogElement | undefined = $state();
  let snoozeSliderMinutes = $state(10);

  function markBabyHeartbeat(): void {
    lastBabyHeartbeatAt = Date.now();
    babyLinkAlertSent = false;
  }

  const room = $derived(page.url.searchParams.get("room") ?? "");
  const signalQ = $derived(page.url.searchParams.get("signal"));
  const signalingUrl = $derived(resolveSignalingUrlForSession(signalQ));

  const reconnectCountdownSec = $derived.by(() => {
    reconnectTick;
    if (reconnectEtaMs <= 0) return 0;
    return Math.max(0, Math.ceil((reconnectEtaMs - Date.now()) / 1000));
  });

  const snoozeActive = $derived(snoozeUntil > Date.now());

  $effect(() => {
    if (!browser) return;
    const r = room;
    const u = signalingUrl;
    const _lanPeers = lanPeerMap;
    if (!r || validateSignalingUrl(u) !== null) return;
    const t = window.setTimeout(() => start(), 0);
    return () => clearTimeout(t);
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
    if (!browser) return;
    if (status !== "connected") return;
    markBabyHeartbeat();
    const staleMs = 50_000;
    const id = window.setInterval(() => {
      if (Date.now() - lastBabyHeartbeatAt > staleMs && !babyLinkAlertSent) {
        babyLinkAlertSent = true;
        void notifyBabyLinkUncertain();
      }
    }, 4000);
    return () => clearInterval(id);
  });

  $effect(() => {
    if (!browser || !isNativeAndroid() || !room) return;
    const acc: Record<string, LanPeerEntry> = {};
    void GuardedSleepNative.startNsdDiscovery().catch(() => {});
    void GuardedSleepNative.addListener("lanPeerResolved", (e) => {
      const host = e.host;
      const port = typeof e.port === "number" && e.port > 0 ? e.port : 9;
      const roomFrag = typeof e.room === "string" ? e.room : "";
      const token = typeof e.token === "string" ? e.token : "";
      const key = `${host}:${port}`;
      acc[key] = { room: roomFrag, host, port, token };
      lanPeerMap = { ...acc };
    }).then((h) => {
      lanListener = h;
    });
    return () => {
      void lanListener?.remove();
      lanListener = undefined;
      void GuardedSleepNative.stopNsdDiscovery();
      lanPeerMap = {};
    };
  });

  $effect(() => {
    if (!browser || !isNativeAndroid()) return;
    return subscribeNetworkHandoff(() => start({ resetBackoff: true }));
  });

  function ensureBabyAudioPlaying(): void {
    const el = audioEl;
    const stream = remoteStream;
    if (!el || !stream) return;
    el.srcObject = stream;
    el.muted = false;
    void el.play().catch(() => {});
  }

  function start(opts?: { resetBackoff?: boolean }): void {
    if (opts?.resetBackoff) {
      sessionReconnectAttempt = 0;
      reconnectEtaMs = 0;
    }
    void ensureParentMonitorNotificationsReady();
    if (listenOn) pendingListenResume = true;
    session?.close();
    if (import.meta.env.DEV) {
      devNoiseAlertCount = 0;
    }
    listenOn = false;
    quality = "unknown";
    remoteStream = null;
    const psk = takePairingPskForRoom(room);
    const pairing = psk != null && psk.length >= 16 ? { roomId: room, pskKey: psk } : undefined;
    const savedFallbacks = listConnections().find(
      (c) => c.roomId === room && c.signalingUrl === signalingUrl,
    )?.fallbackSignalingUrls;
    const lanPrepend = lanPrependSignalingUrls(lanPeerMap, room);
    const sessionOptions: ConstructorParameters<typeof PeerSession>[4] = {
      ...(pairing ? { pairing } : {}),
      ...(lanPrepend.length > 0 ? { prependSignalingUrls: lanPrepend } : {}),
      ...(savedFallbacks && savedFallbacks.length > 0
        ? { fallbackSignalingUrls: savedFallbacks }
        : {}),
    };
    const s = new PeerSession(
      "parent",
      room,
      signalingUrl,
      {
        onState: (st) => {
          status = st;
          if (st === "connected") {
            markBabyHeartbeat();
            upsertConnection({ roomId: room, signalingUrl, label: "Parent" });
            if (pendingListenResume) {
              pendingListenResume = false;
              listenOn = true;
              s.sendListen(true);
            }
          }
          if (st === "idle" || st === "failed") {
            listenOn = false;
            remoteStream = null;
          }
        },
        onRemoteStream: (stream) => {
          remoteStream = stream;
        },
        onDcHeartbeat: (hb) => {
          markBabyHeartbeat();
          if (hb.battery !== undefined) babyBattery = hb.battery;
          if (hb.rms !== undefined) {
            lastRemoteRms = hb.rms;
            if (hb.rms < 0.012) {
              quietSince = new Date(hb.t);
            }
          }
        },
        onDcNoiseAlert: (ev) => {
          if (import.meta.env.DEV) {
            devNoiseAlertCount += 1;
          }
          if (import.meta.env.DEV && devNoiseAlertResolver) {
            const r = devNoiseAlertResolver;
            devNoiseAlertResolver = null;
            r(ev.rms);
          }
          if (Date.now() < snoozeUntil) return;
          try {
            navigator.vibrate?.([180, 120, 180]);
          } catch {
            /* ignore */
          }
          if (browser && isNativeAndroid()) {
            void GuardedSleepNative.bringToForeground();
          }
          ensureBabyAudioPlaying();
          void notifyNoiseAlertIfBackground();
        },
        onConnectionQuality: (q) => {
          quality = q;
        },
      },
      sessionOptions,
    );
    session = s;
    s.start();
  }

  $effect(() => {
    if (!import.meta.env.DEV || !browser) return;
    const api: GuardedsleepDevTestApi = {
      role: "parent",
      getSession: () => session,
      getNoiseAlertCount: () => devNoiseAlertCount,
      resetNoiseAlertCount: () => {
        devNoiseAlertCount = 0;
      },
      waitNextNoiseAlert: (timeoutMs = 5000) =>
        new Promise<number>((resolve, reject) => {
          const ms = timeoutMs ?? 5000;
          const t = window.setTimeout(() => {
            if (devNoiseAlertResolver) devNoiseAlertResolver = null;
            reject(new Error("noise alert timeout"));
          }, ms);
          devNoiseAlertResolver = (rms) => {
            window.clearTimeout(t);
            resolve(rms);
          };
        }),
    };
    window.__guardedsleep_test = api;
    return () => {
      if (window.__guardedsleep_test === api) delete window.__guardedsleep_test;
    };
  });

  async function openBatterySettings(): Promise<void> {
    if (!browser) return;
    if (isNativeAndroid()) {
      await GuardedSleepNative.openBatterySettings();
      return;
    }
    window.alert(i18n?.t("parent.batteryAlertWeb") ?? "");
  }

  function toggleListen(): void {
    const s = session;
    if (!s) return;
    listenOn = !listenOn;
    if (!listenOn) pendingListenResume = false;
    s.sendListen(listenOn);
  }

  function applySnoozeMinutes(minutes: number): void {
    const m = Math.min(60, Math.max(1, Math.round(minutes)));
    snoozeUntil = Date.now() + m * 60_000;
    persistLastSnoozeMinutes(m);
    snoozeDialogEl?.close();
  }

  function openSnoozeDialog(): void {
    snoozeSliderMinutes = readLastSnoozeMinutes();
    snoozeDialogEl?.showModal();
  }

  function clearSnooze(): void {
    snoozeUntil = 0;
  }

  function openBabySettingsOnChild(): void {
    session?.sendOpenBabySettings();
  }

  function qualityLabel(q: "good" | "medium" | "poor" | "unknown"): string {
    const key =
      q === "good"
        ? "parent.qualityGood"
        : q === "medium"
          ? "parent.qualityMedium"
          : q === "poor"
            ? "parent.qualityPoor"
            : "parent.qualityUnknown";
    return i18n?.t(key) ?? q;
  }

  $effect(() => {
    const el = audioEl;
    const s = remoteStream;
    if (el && s) {
      el.srcObject = s;
      el.muted = false;
      void el.play().catch(() => {});
    }
  });

  $effect(() => {
    if (!browser) return;
    const onVis = (): void => {
      if (document.visibilityState === "visible") ensureBabyAudioPlaying();
    };
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  });

  $effect(() => {
    if (!browser || !listenOn) return;
    let lock: WakeLockSentinel | undefined;
    void navigator.wakeLock
      ?.request("screen")
      .then((l) => {
        lock = l;
      })
      .catch(() => {});
    return () => {
      void lock?.release();
    };
  });

  onDestroy(() => session?.close());
</script>

<audio bind:this={audioEl} autoplay playsinline class="hidden"></audio>

<dialog
  bind:this={snoozeDialogEl}
  class="w-[min(100vw-2rem,22rem)] rounded-xl border border-zinc-700 bg-zinc-900 p-4 text-zinc-100 shadow-xl backdrop:bg-black/60"
  aria-labelledby="snooze-dialog-title"
>
  <h2 id="snooze-dialog-title" class="text-base font-semibold text-zinc-50">
    {i18n?.t("parent.snoozeDialogTitle") ?? ""}
  </h2>
  <p class="mt-1 text-xs text-zinc-500">
    {i18n?.t("parent.snoozeDialogHelp") ?? ""}
  </p>
  <div class="mt-4 grid grid-cols-3 gap-2">
    {#each [5, 10, 15] as m (m)}
      <button
        type="button"
        class="rounded-lg bg-zinc-800 py-2 text-sm font-medium text-zinc-100 hover:bg-zinc-700"
        onclick={() => applySnoozeMinutes(m)}
      >
        {i18n?.t("parent.snoozeMinutesShort", { n: m }) ?? ""}
      </button>
    {/each}
  </div>
  <div class="mt-4 border-t border-zinc-800 pt-4">
    <label class="flex flex-col gap-2 text-sm">
      <span class="text-zinc-400"
        >{i18n?.t("parent.snoozeCustomLabel", { minutes: snoozeSliderMinutes }) ?? ""}</span
      >
      <input
        type="range"
        min="1"
        max="60"
        step="1"
        bind:value={snoozeSliderMinutes}
        class="min-h-11 w-full"
      />
    </label>
    <button
      type="button"
      class="mt-3 w-full rounded-lg bg-emerald-700 py-2 text-sm font-medium text-white hover:bg-emerald-600"
      onclick={() => applySnoozeMinutes(snoozeSliderMinutes)}
    >
      {i18n?.t("parent.snoozeApply", { minutes: snoozeSliderMinutes }) ?? ""}
    </button>
  </div>
  <button
    type="button"
    class="mt-3 w-full rounded-lg border border-zinc-600 py-2 text-sm text-zinc-300 hover:bg-zinc-800"
    onclick={() => snoozeDialogEl?.close()}
  >
    {i18n?.t("parent.snoozeCancel") ?? ""}
  </button>
</dialog>

<div class="mx-auto flex max-w-md flex-col gap-4 px-4 py-8">
  <ConnectionStatusBar
    role="parent"
    {status}
    {reconnectCountdownSec}
    reconnectAttempt={sessionReconnectAttempt}
    maxAttempts={SESSION_AUTO_RECONNECT_MAX_ATTEMPTS}
  />
  <header class="flex flex-wrap items-center justify-between gap-2">
    <h1 class="text-xl font-semibold text-zinc-50">{i18n?.t("parent.title") ?? ""}</h1>
    <div class="flex flex-wrap items-center gap-2">
      {#if status === "connected" && !remoteStream}
        <span
          class="max-w-[12rem] rounded-full bg-amber-950/60 px-2 py-1 text-[11px] font-medium text-amber-100 ring-1 ring-amber-800/50"
          title={i18n?.t("parent.pillSessionUpTitle") ?? ""}
        >
          {i18n?.t("parent.pillSessionUp") ?? ""}
        </span>
      {:else if status === "connected" && remoteStream}
        <span
          class="rounded-full bg-emerald-950/70 px-2 py-1 text-[11px] font-medium text-emerald-100 ring-1 ring-emerald-800/50"
          title={i18n?.t("parent.pillAudioReadyTitle") ?? ""}
        >
          {i18n?.t("parent.pillAudioReady") ?? ""}
        </span>
      {/if}
      <span
        class="rounded-full bg-zinc-800 px-2 py-1 text-xs text-zinc-300"
        data-testid="connection-status"
        data-session-state={status}>{i18n?.t(connectionStatusPillI18nKey(status)) ?? status}</span
      >
    </div>
  </header>
  <p class="text-sm text-zinc-500">
    {i18n?.t("parent.roomLabel") ?? ""} <span class="font-mono text-zinc-200">{room || "—"}</span>
  </p>
  {#if room}
    <p class="text-xs leading-relaxed text-zinc-500">
      {#if status === "connected"}
        {i18n?.t("parent.blurbConnected") ?? ""}
      {:else if status === "connecting"}
        {i18n?.t("parent.blurbConnecting") ?? ""}
      {:else if status === "failed"}
        {i18n?.t("parent.blurbFailed") ?? ""}
      {:else}
        {i18n?.t("parent.blurbIdle") ?? ""}
      {/if}
    </p>
  {/if}
  {#if !room}
    <p class="text-amber-300">
      {i18n?.t("parent.missingRoom") ?? ""}
      <a class="text-emerald-400 underline" href="/pair?role=parent"
        >{i18n?.t("parent.configurePairing") ?? ""}</a
      >
    </p>
  {:else}
    {#if status === "failed"}
      <div
        class="rounded-lg border border-amber-800/80 bg-amber-950/25 px-3 py-3 text-sm text-amber-100/95"
        role="status"
      >
        <p class="font-medium text-amber-100">{i18n?.t("parent.failedBannerTitle") ?? ""}</p>
        <p class="mt-1 text-xs text-amber-200/80">
          {i18n?.t("parent.failedBannerHint") ?? ""}
        </p>
        {#if sessionReconnectAttempt > 0 && sessionReconnectAttempt <= SESSION_AUTO_RECONNECT_MAX_ATTEMPTS}
          <p class="mt-2 text-xs text-amber-200/90">
            {i18n?.t("parent.retryLine", {
              seconds: reconnectCountdownSec,
              attempt: sessionReconnectAttempt,
              max: SESSION_AUTO_RECONNECT_MAX_ATTEMPTS,
            }) ?? ""}
          </p>
        {:else if sessionReconnectAttempt > SESSION_AUTO_RECONNECT_MAX_ATTEMPTS}
          <p class="mt-2 text-xs text-amber-200/90">{i18n?.t("parent.retriesStopped") ?? ""}</p>
        {/if}
        <button
          type="button"
          class="mt-3 w-full rounded-lg bg-amber-600 px-3 py-2 text-sm font-medium text-amber-950 hover:bg-amber-500"
          onclick={() => start({ resetBackoff: true })}
        >
          {i18n?.t("parent.tryNow") ?? ""}
        </button>
      </div>
    {/if}

    {#if Object.keys(lanPeerMap).length > 0}
      <div class="rounded-lg border border-zinc-800 bg-zinc-900/40 p-3 text-xs text-zinc-400">
        <p class="mb-1 font-medium text-zinc-500">{i18n?.t("parent.lanDiscoveryTitle") ?? ""}</p>
        <ul class="space-y-1 font-mono text-[11px] text-zinc-300">
          {#each Object.values(lanPeerMap) as p}
            <li>
              {p.host}:{p.port}
              {#if p.token}
                {i18n?.t("parent.lanRowToken", { suffix: p.token.slice(-6) })}
              {:else}
                {i18n?.t("parent.lanRowRoom", { fragment: p.room || "—" })}
              {/if}
            </li>
          {/each}
        </ul>
      </div>
    {/if}
    <div class="grid grid-cols-2 gap-2 text-sm">
      <div class="rounded-lg border border-zinc-800 bg-zinc-900/60 p-3">
        <p class="text-xs text-zinc-500">{i18n?.t("parent.babyBattery") ?? ""}</p>
        <p class="text-lg font-medium text-zinc-100">
          {babyBattery != null ? `${Math.round(babyBattery * 100)}%` : "—"}
        </p>
      </div>
      <div class="rounded-lg border border-zinc-800 bg-zinc-900/60 p-3">
        <p class="text-xs text-zinc-500">{i18n?.t("parent.linkQuality") ?? ""}</p>
        <p class="text-lg font-medium capitalize text-zinc-100">{qualityLabel(quality)}</p>
      </div>
    </div>
    <div class="rounded-lg border border-zinc-800 bg-zinc-900/50 p-3">
      <p class="mb-1 text-xs text-zinc-500">{i18n?.t("parent.noiseRemoteTitle") ?? ""}</p>
      <div class="h-3 overflow-hidden rounded-full bg-zinc-800">
        <div
          class="h-full bg-emerald-500 transition-[width]"
          style:width="{Math.min(100, (lastRemoteRms ?? 0) * 400)}%"
        ></div>
      </div>
      <p class="mt-1 text-xs text-zinc-500">
        {i18n?.t("parent.allQuietSince") ?? ""}
        <span class="text-zinc-300">
          {quietSince ? quietSince.toLocaleTimeString(undefined, { timeStyle: "short" }) : "—"}
        </span>
      </p>
    </div>
    <div class="flex flex-wrap gap-2">
      <button
        type="button"
        class="flex-1 rounded-lg bg-emerald-600 px-4 py-3 font-medium text-emerald-950 hover:bg-emerald-500"
        data-testid="session-start"
        onclick={() => start({ resetBackoff: true })}
      >
        {i18n?.t("parent.restartSession") ?? ""}
      </button>
      <button
        type="button"
        class={`rounded-lg px-4 py-3 text-sm font-medium ${listenOn ? "bg-amber-600 text-amber-950" : "border border-zinc-600 text-zinc-200 hover:bg-zinc-900"}`}
        onclick={toggleListen}
        disabled={status !== "connected"}
      >
        {listenOn ? (i18n?.t("parent.listenOn") ?? "") : (i18n?.t("parent.listenOff") ?? "")}
      </button>
    </div>
    {#if status === "connected"}
      <div class="rounded-lg border border-zinc-700 bg-zinc-900/40 p-3">
        <p class="text-sm font-medium text-zinc-200">{i18n?.t("parent.babyCardTitle") ?? ""}</p>
        <p class="mt-1 text-xs text-zinc-500">
          {i18n?.t("parent.babyCardHelp") ?? ""}
        </p>
        <button
          type="button"
          class="mt-3 w-full rounded-lg border border-emerald-700/60 bg-emerald-950/40 px-3 py-2 text-sm font-medium text-emerald-200 hover:bg-emerald-950/70"
          onclick={openBabySettingsOnChild}
        >
          {i18n?.t("parent.openBabySettings") ?? ""}
        </button>
      </div>
    {/if}
    {#if snoozeActive}
      <div
        class="flex flex-wrap items-center justify-between gap-2 rounded-lg border border-amber-800/50 bg-amber-950/20 px-3 py-2 text-sm text-amber-100/90"
      >
        <span
          >{i18n?.t("parent.snoozedUntil", {
            time: new Date(snoozeUntil).toLocaleTimeString(undefined, { timeStyle: "short" }),
          }) ?? ""}</span
        >
        <button
          type="button"
          class="rounded-lg border border-amber-700/60 px-2 py-1 text-xs font-medium text-amber-100 hover:bg-amber-900/40"
          onclick={clearSnooze}
        >
          {i18n?.t("parent.clearSnooze") ?? ""}
        </button>
      </div>
    {/if}
    <button
      type="button"
      class="rounded-lg border border-zinc-600 px-4 py-2 text-sm text-zinc-300 hover:bg-zinc-900"
      onclick={openSnoozeDialog}
    >
      {i18n?.t("parent.snoozeAlerts") ?? ""}
    </button>
    {#if isNativeAndroid()}
      <button
        type="button"
        class="rounded-lg border border-zinc-600 px-4 py-2 text-sm text-zinc-300 hover:bg-zinc-900"
        onclick={openBatterySettings}
      >
        {i18n?.t("parent.batterySettings") ?? ""}
      </button>
    {/if}
    <p class="text-xs leading-relaxed text-zinc-600">
      {i18n?.t("parent.footer") ?? ""}
    </p>
  {/if}
</div>
