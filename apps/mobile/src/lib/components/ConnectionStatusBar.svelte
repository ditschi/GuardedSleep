<!--
  ConnectionStatusBar -- ambient connection-status indicator (parent + baby).
  Copy from ADR-013; strings from i18n (connectionStatus.*).
-->
<script lang="ts">
  import { getI18n } from "$lib/i18n/context.js";

  type StatusKind = "idle" | "connecting" | "connected" | "failed";
  type Role = "parent" | "baby";

  type Props = {
    role: Role;
    status: StatusKind;
    reconnectCountdownSec?: number;
    reconnectAttempt?: number;
    maxAttempts?: number;
    peerName?: string;
    testId?: string;
  };

  let {
    role,
    status,
    reconnectCountdownSec = 0,
    reconnectAttempt = 0,
    maxAttempts = 0,
    peerName,
    testId = "connection-status-bar",
  }: Props = $props();

  const i18n = getI18n();

  function tr(key: string, params?: Record<string, string | number>): string {
    return i18n?.t(key, params) ?? key;
  }

  const babyUnit = $derived(tr("connectionStatus.roleBabyUnit"));
  const babyUnitLower = $derived(tr("connectionStatus.roleBabyUnitLower"));
  const parentDevice = $derived(tr("connectionStatus.roleParentDevice"));
  const parentDeviceLower = $derived(tr("connectionStatus.roleParentDeviceLower"));

  const otherUnit = $derived(role === "parent" ? babyUnit : parentDevice);
  const otherUnitLower = $derived(role === "parent" ? babyUnitLower : parentDeviceLower);

  const tone = $derived.by<"green" | "yellow" | "orange" | "red" | "neutral">(() => {
    if (status === "connected") return "green";
    if (status === "connecting") return "yellow";
    if (status === "failed") {
      if (reconnectAttempt > 0 && reconnectAttempt <= maxAttempts) {
        if (reconnectAttempt <= 3) return "yellow";
        if (reconnectAttempt <= 6) return "orange";
        return "red";
      }
      return "red";
    }
    return "neutral";
  });

  const symbol = $derived.by(() => {
    switch (tone) {
      case "green":
        return "🔗";
      case "yellow":
      case "orange":
        return "⏳";
      case "red":
        return "⚠";
      default:
        return "•";
    }
  });

  const primaryText = $derived.by(() => {
    if (status === "connected") {
      if (peerName) return tr("connectionStatus.connectedToName", { name: peerName });
      return role === "parent"
        ? tr("connectionStatus.connectedToBaby")
        : tr("connectionStatus.connectedToParent");
    }
    if (status === "connecting") return tr("connectionStatus.connecting");
    if (status === "idle") return tr("connectionStatus.notMonitoring");
    if (tone === "yellow") return tr("connectionStatus.briefPause");
    if (tone === "orange") return tr("connectionStatus.unitUnreachableTrying", { unit: otherUnit });
    if (reconnectAttempt > maxAttempts && maxAttempts > 0) {
      return tr("connectionStatus.notConnectedTo", { role: otherUnitLower });
    }
    return tr("connectionStatus.unitUnreachable", { unit: otherUnit });
  });

  const subText = $derived.by(() => {
    if (status !== "failed") return "";
    if (reconnectAttempt > 0 && reconnectAttempt <= maxAttempts) {
      const seconds = Math.max(0, reconnectCountdownSec);
      return tr("connectionStatus.nextTry", {
        seconds,
        attempt: reconnectAttempt,
        max: maxAttempts,
      });
    }
    if (reconnectAttempt > maxAttempts && maxAttempts > 0) {
      return role === "parent"
        ? tr("connectionStatus.hintParentGiveUp")
        : tr("connectionStatus.hintBabyGiveUp");
    }
    return "";
  });

  const toneClass = $derived.by(() => {
    switch (tone) {
      case "green":
        return "bg-emerald-900/70 text-emerald-100 ring-emerald-700/60";
      case "yellow":
        return "bg-amber-950/70 text-amber-100 ring-amber-700/60 animate-pulse";
      case "orange":
        return "bg-orange-950/70 text-orange-100 ring-orange-700/60 animate-pulse";
      case "red":
        return "bg-rose-950/80 text-rose-100 ring-rose-700/70 animate-pulse";
      default:
        return "bg-zinc-900 text-zinc-300 ring-zinc-700";
    }
  });
</script>

<div
  class={`flex items-center gap-2 rounded-md px-3 py-2 text-sm ring-1 ${toneClass}`}
  data-testid={testId}
  data-status={status}
  data-tone={tone}
  role="status"
  aria-live="polite"
>
  <span class="text-base" aria-hidden="true">{symbol}</span>
  <div class="flex min-w-0 flex-1 flex-col leading-tight">
    <span class="font-medium">{primaryText}</span>
    {#if subText}
      <span class="text-xs opacity-80" data-testid="{testId}-sub">{subText}</span>
    {/if}
  </div>
</div>
