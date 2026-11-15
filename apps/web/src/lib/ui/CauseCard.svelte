<script lang="ts">
  import { tick } from "svelte";
  import type { CauseHint } from "../contract";
  import type { MessageKey } from "../i18n/keys";
  import { t } from "../i18n/store";
  import Button from "./Button.svelte";

  /**
   * The troubleshooter card of a connection alert (`connection-status` "Troubleshooter
   * presentation"): one cause line, step 1 visible, "Try now" and the further steps behind a
   * disclosure button that carries its expanded state. Focus moves to the first extra step when it
   * opens.
   */
  type Props = {
    hint: CauseHint | null;
    ontrynow: () => void;
  };

  let { hint, ontrynow }: Props = $props();

  interface Copy {
    line: MessageKey | null;
    step: MessageKey;
    more: MessageKey[];
  }

  const COPY: Readonly<Record<CauseHint, Copy>> = {
    peer_absent: {
      line: "native.ui.connAlert.cause.peerAbsent.line",
      step: "native.ui.connAlert.cause.peerAbsent.step",
      more: ["native.ui.connAlert.cause.peerAbsent.more"],
    },
    server_unreachable: {
      line: "native.ui.connAlert.cause.serverUnreachable.line",
      step: "native.ui.connAlert.cause.serverUnreachable.step",
      more: ["native.ui.connAlert.cause.serverUnreachable.more"],
    },
    media_blocked: {
      line: "native.ui.connAlert.cause.mediaBlocked.line",
      step: "native.ui.connAlert.cause.mediaBlocked.step",
      more: ["native.ui.connAlert.cause.mediaBlocked.more"],
    },
    // Pairing causes name the fix in the core's own message; there is nothing to fold.
    pair_again: { line: null, step: "core.pairAgain", more: [] },
    baby_unrecognised: { line: null, step: "core.babyUnrecognised", more: [] },
  };
  const GENERIC: Copy = {
    line: null,
    step: "native.ui.help.reconnect.step1",
    more: ["native.ui.help.reconnect.step2", "native.ui.help.reconnect.step3"],
  };

  const copy = $derived(hint === null ? GENERIC : COPY[hint]);
  let open = $state(false);
  let card: HTMLElement | undefined = $state();

  async function toggle(): Promise<void> {
    open = !open;
    if (open) {
      await tick();
      card?.querySelector<HTMLElement>('[data-testid="link_alarm_more_0"]')?.focus();
    }
  }
</script>

<section class="gs-alert__card" data-testid="link_alarm_cause_card" bind:this={card}>
  {#if copy.line}
    <span class="gs-alert__card-label">{$t("native.ui.connAlert.causeLabel")}</span>
    <p class="gs-alert__card-line" data-testid="link_alarm_cause" data-cause={hint}>
      {$t(copy.line)}
    </p>
  {/if}
  <p data-testid="link_alarm_step" data-cause={hint}>{$t(copy.step)}</p>
  {#if open}
    {#each copy.more as key, i (key)}
      <p tabindex="-1" data-testid={`link_alarm_more_${i}`}>{$t(key)}</p>
    {/each}
  {/if}
  <Button variant="alert-outline" block testid="link_alarm_try_now" onclick={ontrynow}>
    {$t("native.ui.link.tryNow")}
  </Button>
  {#if copy.more.length > 0}
    <button
      type="button"
      class="gs-btn"
      data-variant="text"
      data-testid="link_alarm_more_steps"
      aria-expanded={open}
      onclick={toggle}
    >
      {open ? $t("native.ui.connAlert.fewerSteps") : $t("native.ui.connAlert.moreSteps")}
    </button>
  {/if}
</section>
