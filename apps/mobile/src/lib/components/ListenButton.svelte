<script lang="ts">
  import { getI18n } from "$lib/i18n/context.js";
  import { persistAppPreferences, readAppPreferences } from "$lib/app-preferences.js";
  import { nextPhaseAfterRelease, shouldLockOnSlide } from "$lib/ui/listen-gesture.js";

  const SLIDE_LOCK_THRESHOLD_PX = 48;

  type Props = {
    disabled?: boolean;
    /** Parent-owned: stream is actively playing (hold or locked). */
    listening?: boolean;
    /** Parent-owned: slide-to-lock engaged. */
    locked?: boolean;
    onListeningChange?: (listening: boolean) => void;
    onLockedChange?: (locked: boolean) => void;
  };

  let {
    disabled = false,
    listening = false,
    locked = false,
    onListeningChange,
    onLockedChange,
  }: Props = $props();

  const i18n = getI18n();

  let phase = $state<"idle" | "holding" | "locked">("idle");
  let pointerStartY = 0;
  let slideProgress = $state(0);
  let showHint = $state(!readAppPreferences().listenHintSeen);

  $effect(() => {
    if (!listening && !locked) {
      phase = "idle";
      slideProgress = 0;
    } else if (locked) {
      phase = "locked";
    }
  });

  function dismissHint(): void {
    if (!showHint) return;
    showHint = false;
    persistAppPreferences({ listenHintSeen: true });
  }

  function setListening(on: boolean): void {
    onListeningChange?.(on);
  }

  function setLocked(on: boolean): void {
    onLockedChange?.(on);
  }

  function stopAll(): void {
    setLocked(false);
    setListening(false);
    phase = "idle";
    slideProgress = 0;
  }

  function onPointerDown(ev: PointerEvent): void {
    if (disabled) return;
    const el = ev.currentTarget as HTMLElement;
    el.setPointerCapture(ev.pointerId);

    if (locked || phase === "locked") {
      stopAll();
      dismissHint();
      return;
    }

    phase = "holding";
    pointerStartY = ev.clientY;
    slideProgress = 0;
    setListening(true);
    dismissHint();
  }

  function onPointerMove(ev: PointerEvent): void {
    if (phase !== "holding" || locked) return;
    const deltaY = ev.clientY - pointerStartY;
    slideProgress = Math.min(1, Math.max(0, -deltaY / SLIDE_LOCK_THRESHOLD_PX));
    if (shouldLockOnSlide(deltaY, SLIDE_LOCK_THRESHOLD_PX)) {
      phase = "locked";
      setLocked(true);
      slideProgress = 1;
    }
  }

  function onPointerUp(ev: PointerEvent): void {
    const el = ev.currentTarget as HTMLElement;
    if (el.hasPointerCapture(ev.pointerId)) {
      el.releasePointerCapture(ev.pointerId);
    }
    if (phase === "holding" && !locked) {
      phase = nextPhaseAfterRelease("holding", false);
      setListening(false);
      slideProgress = 0;
    }
  }

  const label = $derived.by(() => {
    if (locked || phase === "locked") return i18n?.t("listenButton.locked") ?? "";
    if (phase === "holding") return i18n?.t("listenButton.holding") ?? "";
    return i18n?.t("listenButton.idle") ?? "";
  });

  const buttonClass = $derived.by(() => {
    if (locked || phase === "locked") {
      return "bg-amber-600 text-amber-950 ring-2 ring-amber-400/60";
    }
    if (phase === "holding") {
      return "bg-emerald-500 text-emerald-950 scale-[1.02] ring-2 ring-emerald-300/50";
    }
    return "border border-zinc-600 bg-zinc-900 text-zinc-100 hover:bg-zinc-800";
  });
</script>

<div class="flex flex-col items-stretch gap-1">
  <button
    type="button"
    class={`relative flex min-h-[3.25rem] flex-col items-center justify-center rounded-xl px-4 py-3 text-sm font-semibold transition-[transform,background-color,box-shadow] select-none touch-none ${buttonClass} ${disabled ? "cursor-not-allowed opacity-50" : ""}`}
    data-testid="parent_listen"
    data-listen-phase={locked ? "locked" : phase}
    data-listening={listening ? "true" : "false"}
    {disabled}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
  >
    {#if phase === "holding" && !locked}
      <span
        class="pointer-events-none absolute inset-x-4 top-1 h-1 overflow-hidden rounded-full bg-emerald-950/40"
        aria-hidden="true"
      >
        <span
          class="block h-full rounded-full bg-emerald-200 transition-[width]"
          style:width="{slideProgress * 100}%"
        ></span>
      </span>
    {/if}
    <span class="flex items-center gap-2">
      {#if locked || phase === "locked"}
        <span aria-hidden="true">🔒</span>
      {:else}
        <span aria-hidden="true">🎧</span>
      {/if}
      {label}
    </span>
  </button>
  {#if showHint && !disabled}
    <p class="text-center text-xs text-zinc-500" data-testid="parent_listen_hint">
      {i18n?.t("listenButton.hint") ?? ""}
    </p>
  {/if}
</div>
