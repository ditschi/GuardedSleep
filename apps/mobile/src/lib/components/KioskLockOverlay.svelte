<script lang="ts">
  import { browser } from "$app/environment";
  import { getI18n } from "$lib/i18n/context.js";
  import { shouldCompleteSlideUp, slideUpProgress } from "$lib/ui/slide-up-gesture.js";

  const SLIDE_THRESHOLD_PX = 72;
  const HOLD_DELAY_MS = 160;
  const TOUCH_WAKE_MS = 3000;
  const DIM_AFTER_MS = 5000;

  type ConnectionKind = "idle" | "connecting" | "connected" | "failed";

  type Props = {
    connectionStatus?: ConnectionKind;
    batteryFraction?: number;
    onUnlock?: () => void;
  };

  let { connectionStatus = "idle", batteryFraction, onUnlock }: Props = $props();

  const i18n = getI18n();

  type Phase = "dimmed" | "awake" | "holding";

  let phase = $state<Phase>("dimmed");
  let slideProgress = $state(0);
  let showUnlockZone = $state(false);

  let pointerStartY = 0;
  let holdTimer: number | undefined;
  let wakeTimer: number | undefined;
  let dimTimer: number | undefined;

  const signalTone = $derived.by(() => {
    if (connectionStatus === "connected") return "text-emerald-400/80";
    if (connectionStatus === "connecting") return "text-amber-400/80";
    return "text-rose-400/80";
  });

  const batteryPct = $derived(
    batteryFraction != null ? Math.round(batteryFraction * 100) : undefined,
  );

  const batteryTone = $derived.by(() => {
    if (batteryPct == null) return "text-zinc-500/70";
    if (batteryPct < 15) return "text-rose-400/80";
    if (batteryPct < 30) return "text-amber-400/80";
    return "text-emerald-400/70";
  });

  const iconOpacity = $derived(phase === "dimmed" ? "opacity-25" : "opacity-100");

  function clearTimers(): void {
    if (holdTimer != null) clearTimeout(holdTimer);
    if (wakeTimer != null) clearTimeout(wakeTimer);
    if (dimTimer != null) clearTimeout(dimTimer);
    holdTimer = undefined;
    wakeTimer = undefined;
    dimTimer = undefined;
  }

  function scheduleDim(): void {
    if (dimTimer != null) clearTimeout(dimTimer);
    dimTimer = window.setTimeout(() => {
      if (phase !== "holding") phase = "dimmed";
      showUnlockZone = false;
      slideProgress = 0;
    }, DIM_AFTER_MS);
  }

  function touchWake(): void {
    phase = "awake";
    showUnlockZone = false;
    slideProgress = 0;
    if (wakeTimer != null) clearTimeout(wakeTimer);
    wakeTimer = window.setTimeout(() => {
      if (phase === "awake") {
        phase = "dimmed";
        scheduleDim();
      }
    }, TOUCH_WAKE_MS);
  }

  function onPointerDown(ev: PointerEvent): void {
    if (!browser) return;
    const el = ev.currentTarget as HTMLElement;
    el.setPointerCapture(ev.pointerId);
    pointerStartY = ev.clientY;
    slideProgress = 0;
    if (holdTimer != null) clearTimeout(holdTimer);
    holdTimer = window.setTimeout(() => {
      phase = "holding";
      showUnlockZone = true;
      if (wakeTimer != null) clearTimeout(wakeTimer);
    }, HOLD_DELAY_MS);
  }

  function onPointerMove(ev: PointerEvent): void {
    if (phase !== "holding") return;
    const deltaY = ev.clientY - pointerStartY;
    slideProgress = slideUpProgress(deltaY, SLIDE_THRESHOLD_PX);
    if (shouldCompleteSlideUp(deltaY, SLIDE_THRESHOLD_PX)) {
      clearTimers();
      onUnlock?.();
    }
  }

  function onPointerUp(ev: PointerEvent): void {
    const el = ev.currentTarget as HTMLElement;
    if (el.hasPointerCapture(ev.pointerId)) {
      el.releasePointerCapture(ev.pointerId);
    }
    if (holdTimer != null) {
      clearTimeout(holdTimer);
      holdTimer = undefined;
      if (phase !== "holding") {
        touchWake();
        return;
      }
    }
    if (phase === "holding") {
      phase = "awake";
      showUnlockZone = false;
      slideProgress = 0;
      wakeTimer = window.setTimeout(() => {
        phase = "dimmed";
        scheduleDim();
      }, TOUCH_WAKE_MS);
    }
  }

  $effect(() => {
    phase = "dimmed";
    showUnlockZone = false;
    slideProgress = 0;
    if (!browser) return;
    scheduleDim();
    return () => clearTimers();
  });
</script>

<div
  class="fixed inset-0 z-50 flex touch-none select-none flex-col bg-black"
  data-testid="kiosk-overlay"
  data-kiosk-phase={phase}
  role="presentation"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
  onpointercancel={onPointerUp}
>
  <div class="flex flex-1 flex-col items-center justify-center gap-2 px-6">
    <div
      class={`flex items-center gap-3 text-sm transition-opacity duration-[5s] ${iconOpacity}`}
      aria-hidden="true"
    >
      <span class={signalTone} title={connectionStatus}>📶</span>
      <span class={batteryTone} title={batteryPct != null ? `${batteryPct}%` : ""}>🔋</span>
      {#if phase !== "dimmed" && batteryPct != null}
        <span class={`text-xs ${batteryTone}`}>{batteryPct}%</span>
      {/if}
    </div>

    {#if phase !== "dimmed"}
      <p class="text-center text-xs text-zinc-500">{i18n?.t("kiosk.monitoringActive") ?? ""}</p>
    {/if}

    {#if phase === "awake" && !showUnlockZone}
      <p class="mt-4 text-center text-sm text-zinc-400" data-testid="kiosk-unlock-hint">
        {i18n?.t("kiosk.holdSlideHint") ?? ""}
      </p>
    {/if}
  </div>

  {#if showUnlockZone}
    <div
      class="mx-4 mb-8 rounded-2xl border border-emerald-700/50 bg-emerald-950/40 px-4 py-4 text-center"
      data-testid="kiosk-unlock-zone"
    >
      <p class="text-sm font-medium text-emerald-200">{i18n?.t("kiosk.unlockZone") ?? ""}</p>
      <div class="mx-auto mt-3 h-1.5 w-full overflow-hidden rounded-full bg-emerald-950">
        <div
          class="h-full rounded-full bg-emerald-400 transition-[width]"
          style:width="{slideProgress * 100}%"
        ></div>
      </div>
    </div>
  {/if}
</div>
