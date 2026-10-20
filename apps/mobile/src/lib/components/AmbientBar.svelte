<script lang="ts">
  import { getI18n } from "$lib/i18n/context.js";
  import { formatClockDuration, formatDuration } from "$lib/link/format.js";
  import type { LinkUi } from "$lib/link/ui-contract.js";

  type Props = {
    link: LinkUi;
    role: "parent" | "baby";
    testid: string;
    /** Driver clock, ticking about once per second. */
    nowElapsedMs: number;
    /** Used for "no connection for" when the peer was never reachable in this run. */
    startedElapsedMs: number;
    onTryNow?: () => void;
    onChecking?: () => void;
    /** Baby unit in a dark room: dim colours, slow pulse, never bright. */
    dim?: boolean;
  };

  let {
    link,
    role,
    testid,
    nowElapsedMs,
    startedElapsedMs,
    onTryNow,
    onChecking,
    dim = false,
  }: Props = $props();

  const i18n = getI18n();
  const t = (key: string, params?: Record<string, string | number>): string =>
    i18n?.t(key, params) ?? key;

  const down = $derived(Math.max(0, nowElapsedMs - (link.lastAliveElapsedMs ?? startedElapsedMs)));
  const muted = $derived(
    link.acknowledgedUntilElapsedMs != null && link.acknowledgedUntilElapsedMs > nowElapsedMs,
  );

  type Tone = { cls: string; symbol: string; text: string };

  const tone = $derived.by((): Tone => {
    switch (link.phase) {
      case "connected":
        return {
          cls: "bg-emerald-950 text-emerald-100 ring-emerald-700",
          symbol: "🔗",
          text: t(`web.bar.${role}.connected`),
        };
      case "peer_stopped":
        return {
          cls: "bg-zinc-800 text-zinc-200 ring-zinc-600",
          symbol: "⏸",
          text: t(`web.bar.${role}.peerStopped`),
        };
      case "failed_needs_user":
        return {
          cls: "bg-red-950 text-red-100 ring-red-700",
          symbol: "✕ !",
          text:
            link.failureKey === "update_required"
              ? t("web.bar.updateRequired")
              : link.failureKey === "browser_unsupported"
                ? t("web.bar.browserUnsupported")
                : t("web.bar.needsPairing"),
        };
      case "connecting":
      case "reconnecting": {
        if (link.level >= 3) {
          return {
            cls: "bg-red-800 text-white ring-red-300 motion-safe:animate-pulse",
            symbol: "✕ !",
            text: t("web.bar.lostFor", { duration: formatDuration(down) }),
          };
        }
        if (link.level >= 2) {
          return {
            cls: "bg-red-950 text-red-100 ring-red-600",
            symbol: "✕ !",
            text: t("web.bar.lostFor", { duration: formatDuration(down) }),
          };
        }
        if (link.level === 1) {
          return {
            cls: "bg-orange-950 text-orange-100 ring-orange-600",
            symbol: "✕",
            text: t(`web.bar.${role}.cantReach`),
          };
        }
        const first = link.phase === "connecting";
        return {
          cls: "bg-yellow-950 text-yellow-100 ring-yellow-700",
          symbol: "↻",
          text: first ? t(`web.bar.${role}.connecting`) : t(`web.bar.${role}.reconnecting`),
        };
      }
      default:
        return {
          cls: "bg-zinc-800 text-zinc-300 ring-zinc-600",
          symbol: "⏸",
          text: t("web.bar.idle"),
        };
    }
  });

  const causeKey = $derived(
    link.causeHint === "server_unreachable"
      ? "web.cause.serverUnreachable"
      : link.causeHint === "peer_absent"
        ? role === "parent"
          ? "web.cause.peerAbsent"
          : "web.cause.peerAbsentBaby"
        : link.causeHint === "media_blocked"
          ? "web.cause.mediaBlocked"
          : link.causeHint === "update_required"
            ? "web.bar.updateRequired"
            : null,
  );

  const barClass = $derived.by(() => {
    if (!dim) return tone.cls;
    if (link.phase === "connected") return "bg-black text-emerald-900 ring-zinc-900";
    if ((link.phase === "connecting" || link.phase === "reconnecting") && link.level >= 2) {
      return "bg-[#1a0000] text-red-900 ring-red-950 motion-safe:animate-[pulse_10s_ease-in-out_infinite]";
    }
    if (link.level === 1) return "bg-black text-orange-900 ring-zinc-900";
    return "bg-black text-yellow-900 ring-zinc-900";
  });

  const troubleshooting = $derived(
    (link.phase === "connecting" || link.phase === "reconnecting") && link.level >= 2,
  );
  const showTryNow = $derived(
    (link.phase === "connecting" || link.phase === "reconnecting") && link.level >= 1,
  );
</script>

<div
  class={`rounded-xl px-4 py-3 ring-1 ${barClass}`}
  data-testid={testid}
  data-state={link.phase}
  data-level={link.level}
  role="status"
  aria-live={link.level >= 2 ? "assertive" : "polite"}
>
  <p class="flex items-center gap-2 text-base font-medium">
    <span aria-hidden="true">{tone.symbol}</span>
    <span data-testid="link_bar_text">{tone.text}</span>
  </p>
  {#if showTryNow && link.level < 2}
    <p class="mt-1 text-xs opacity-90" data-testid="link_unreachable_for">
      {t("web.bar.notReachableFor", { time: formatClockDuration(down) })}
    </p>
  {/if}
  {#if troubleshooting}
    {#if causeKey}
      <p class="mt-2 text-sm" data-testid="link_cause" data-cause={link.causeHint}>
        {t(causeKey)}
      </p>
    {/if}
    <ul class="mt-2 list-disc space-y-1 ps-5 text-xs opacity-90" data-testid="link_help">
      <li>{t("web.help.step1")}</li>
      <li>{t("web.help.step2")}</li>
      <li>{t("web.help.step3")}</li>
    </ul>
  {/if}
  {#if showTryNow || (role === "parent" && link.level >= 2)}
    <div class="mt-3 flex flex-wrap gap-2">
      {#if showTryNow && onTryNow}
        <button
          type="button"
          class="min-h-12 rounded-lg bg-white/10 px-4 text-sm font-medium hover:bg-white/20"
          data-testid="link_try_now"
          onclick={onTryNow}>{t("web.bar.tryNow")}</button
        >
      {/if}
      {#if role === "parent" && link.level >= 2 && onChecking && link.phase !== "peer_stopped"}
        <button
          type="button"
          class="min-h-12 rounded-lg bg-white px-4 text-sm font-semibold text-red-950 hover:bg-red-100"
          data-testid="link_alarm_ack"
          onclick={onChecking}>{t("web.bar.checking")}</button
        >
      {/if}
    </div>
    {#if muted}
      <p class="mt-2 text-xs opacity-90" data-testid="link_alarm_ack_note">
        {t("web.bar.checkingNote")}
      </p>
    {/if}
  {/if}
</div>
