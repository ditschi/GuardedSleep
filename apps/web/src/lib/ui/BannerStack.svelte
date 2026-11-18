<script lang="ts">
  import { useController } from "../contract";
  import type { ReadinessIssue } from "../contract";
  import { locale, t } from "../i18n/store";
  import { READINESS_WORDS, SILENCE_TITLE, UNIT_WARNING_KEYS } from "./banner-copy";
  import {
    bannerLevel,
    browserBanners,
    parentBanners,
    rankBanners,
    type Banner,
    type BannerKind,
  } from "./banners";
  import Button from "./Button.svelte";
  import Dialog from "./Dialog.svelte";
  import { formatClock, formatDuration } from "./format";
  import Glyph from "./Glyph.svelte";
  import { canRequestSilence } from "./parent-view";
  import { everySecond } from "./ticker";

  /**
   * The notices on the parent monitor (`parent-alerts` "Banner priority on the monitor"): at most
   * two cards, ranked, and one "+N more" row that opens the rest. Each card takes its colour from
   * the fixed `--safety-*` tokens by level, so night mode cannot recolour it. A new warn or alert
   * card is announced once through its live region (it appears with the card and stays quiet
   * while the card stays). The browser has no readiness checker, so a "Fix" exists only where this
   * page can act.
   */
  type Props = {
    /** "Fix" on the "no recent test alert" item opens the test sheet. */
    ontest: () => void;
  };

  let { ontest }: Props = $props();

  const controller = useController();
  const { parent, pageGuard, soundUnlocked } = controller;

  // The browser always keeps the "sound" alert style (it cannot vibrate).
  const ranked = $derived(
    rankBanners([
      ...parentBanners($parent, "sound", $everySecond.wallMs),
      ...browserBanners($pageGuard, $soundUnlocked),
    ]),
  );
  let listOpen = $state(false);
  $effect(() => {
    if (ranked.folded.length === 0) listOpen = false;
  });

  interface Words {
    title: string;
    body: string | null;
  }

  function wordsOf(b: Banner): Words | null {
    const p = $parent;
    const kind: BannerKind = b.kind;
    switch (kind) {
      case "security":
        return {
          title: $t("native.ui.parent.securityTitle"),
          body: $t("native.ui.parent.securityBody"),
        };
      case "mic_silent":
        return b.id === "mic_permission_lost"
          ? { title: $t("native.ui.parent.unitWarning.micPermissionLost"), body: null }
          : {
              title: $t("native.ui.banner.micSilentTitle"),
              body: $t("native.ui.banner.micSilentBody"),
            };
      case "readiness": {
        const w = READINESS_WORDS[b.id as ReadinessIssue];
        return { title: $t(w.title), body: w.body ? $t(w.body) : null };
      }
      case "baby_battery_low":
        return {
          title: $t("native.ui.battery.babyLow", { percent: p.peer.batteryPct ?? 0 }),
          body: null,
        };
      case "baby_unplugged":
        return { title: $t("native.ui.parent.unitWarning.notCharging"), body: null };
      case "silence":
        return {
          title: $t(SILENCE_TITLE[p.peer.silence] ?? "native.ui.silence.mayRing"),
          // The failed request is said where the user pressed "Fix".
          body: p.silenceRequest === "failed" ? $t("native.ui.silence.failed") : null,
        };
      case "unit_warning": {
        const key = UNIT_WARNING_KEYS[b.id.slice("unit_warning_".length)];
        return key ? { title: $t(key), body: null } : null;
      }
      case "weak_connection":
        return { title: $t("native.ui.link.weak"), body: null };
      case "paused":
        return {
          title: $t("native.ui.snooze.bannerTitle", {
            time: formatClock(p.snoozedUntilWallMs ?? $everySecond.wallMs, $locale),
          }),
          body: $t("native.ui.snooze.bannerBody"),
        };
      case "vibrate_only":
        return {
          title: $t("native.ui.alertStyle.vibrateTitle"),
          body: $t("native.ui.alertStyle.bodyNoSound"),
        };
      case "browser_sound_gate":
        return { title: $t("webScreens.parent.soundGateBanner"), body: null };
      case "browser_keep_open":
        if (b.id.endsWith("lost"))
          return { title: $t("webScreens.guard.wakeLockLost"), body: null };
        if (b.id.endsWith("unsupported")) {
          return { title: $t("webScreens.guard.wakeLockUnsupported"), body: null };
        }
        return {
          title: $t("webScreens.parent.keepOpenTitle"),
          body: $t("webScreens.parent.keepOpenBody"),
        };
      case "browser_away": {
        const away = $pageGuard.away;
        if (away === null) return null;
        return {
          title: $t(
            away.kind === "sleep"
              ? "webScreens.guard.awaySleep"
              : "webScreens.guard.awayBackground",
            { duration: formatDuration($t, away.awayMs) },
          ),
          body: null,
        };
      }
    }
  }

  interface Action {
    label: string;
    run: () => void;
    disabled?: boolean;
  }

  function actionOf(b: Banner): Action | null {
    const p = $parent;
    switch (b.kind) {
      case "readiness":
        return b.id === "never_tested" ? { label: $t("native.ui.banner.fix"), run: ontest } : null;
      case "silence":
        return canRequestSilence(p)
          ? {
              label:
                p.silenceRequest === "pending"
                  ? $t("native.ui.silence.pending")
                  : $t("native.ui.banner.fix"),
              run: () => controller.requestSilenceBaby(),
              disabled: p.silenceRequest === "pending",
            }
          : null;
      case "paused":
        return { label: $t("native.ui.snooze.resume"), run: () => controller.cancelSnooze() };
      case "browser_sound_gate":
        return {
          label: $t("webScreens.parent.soundGateTurnOn"),
          run: () => controller.unlockSound(),
        };
      case "browser_keep_open":
        return b.id.endsWith("lost")
          ? { label: $t("webScreens.guard.wakeLockRetry"), run: () => controller.retryKeepAwake() }
          : null;
      case "browser_away":
        return { label: $t("native.ui.common.ok"), run: () => controller.dismissAwayReport() };
      default:
        return null;
    }
  }

  const foldedTitles = $derived(
    ranked.folded.map((b) => wordsOf(b)?.title).filter((x): x is string => x !== undefined),
  );
</script>

{#snippet card(b: Banner, live: boolean)}
  {@const words = wordsOf(b)}
  {@const action = actionOf(b)}
  {@const level = bannerLevel(b.kind, b.id)}
  {#if words}
    <div
      class="gs-notice"
      data-level={level}
      data-kind={b.kind}
      data-state={b.kind === "browser_keep_open"
        ? b.id.slice("browser_keep_open".length + 1) || undefined
        : undefined}
      data-away={b.kind === "browser_away" ? $pageGuard.away?.kind : undefined}
      data-testid={`parent_banner_${b.id}`}
      role={!live
        ? undefined
        : level === "alert"
          ? "alert"
          : level === "warn"
            ? "status"
            : undefined}
    >
      <Glyph name={level === "alert" || level === "warn" ? "alert" : "info"} />
      <div class="gs-notice__text">
        <p class="gs-notice__title">{words.title}</p>
        {#if words.body}
          <p
            class="gs-notice__body"
            data-testid={b.kind === "silence" ? "sheet_silence_failed" : undefined}
          >
            {words.body}
          </p>
        {/if}
      </div>
      {#if action}
        <Button
          variant="outline"
          disabled={action.disabled}
          testid={`parent_banner_${b.id}_action`}
          onclick={action.run}
        >
          {action.label}
        </Button>
      {/if}
    </div>
  {/if}
{/snippet}

{#if ranked.shown.length > 0}
  <div class="gs-notices" data-testid="parent_banners">
    {#each ranked.shown as b (b.id)}
      {@render card(b, true)}
    {/each}
    {#if ranked.folded.length > 0}
      <button
        type="button"
        class="gs-notices__more"
        data-testid="parent_banner_more"
        aria-label={$t("native.ui.banner.moreA11y", {
          count: ranked.folded.length,
          notices: foldedTitles.join(", "),
        })}
        onclick={() => (listOpen = true)}
      >
        {$t("native.ui.banner.more", { count: ranked.folded.length })}
      </button>
    {/if}
  </div>
{/if}

<Dialog status bind:open={listOpen} title={$t("native.ui.banner.listTitle")} testid="sheet_banners">
  {#if listOpen}
    <div class="gs-notices">
      {#each ranked.folded as b (b.id)}
        {@render card(b, false)}
      {/each}
    </div>
  {/if}
  {#snippet actions()}
    <Button variant="text" onclick={() => (listOpen = false)}>{$t("native.ui.common.ok")}</Button>
  {/snippet}
</Dialog>
