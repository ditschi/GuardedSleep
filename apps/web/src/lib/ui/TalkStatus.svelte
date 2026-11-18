<script lang="ts">
  import type { WebTalkState } from "../contract";
  import { t } from "../i18n/store";
  import Button from "./Button.svelte";
  import {
    mayAnnounce,
    noiseLine,
    stateLine,
    unavailableLine,
    type TalkHint,
    type TalkView,
  } from "./talk-view";

  /**
   * The status lines of talk-back above the dock: the state line, why talk is unavailable, noise
   * alerts paused, a silent microphone, "may not be audible" and the after-talk listen. While a
   * talk is requested or running, "Listening is paused while you talk" shows as plain text. While
   * the microphone is open nothing here is a live region (no speech into the open microphone); the
   * result is announced after the release.
   */
  type Props = {
    talk: TalkView;
    web: WebTalkState;
    nowMs: number;
    hint: TalkHint | null;
    onkeeplisten: () => void;
  };

  let { talk, web, nowMs, hint, onkeeplisten }: Props = $props();

  const unavailable = $derived(unavailableLine(talk, web, nowMs));
  const stateText = $derived(stateLine(talk, nowMs));
  const noise = $derived(noiseLine(talk));
  const busy = $derived(talk.talk.kind === "requesting" || talk.talk.kind === "talking");
  // A line the user must read after the talk; never while the microphone is open.
  const showUnconfirmed = $derived(talk.talkUnconfirmed && talk.talk.kind !== "talking");
</script>

<div class="gs-talk-status">
  {#if busy}
    <p class="gs-talk__line" data-testid="parent_talk_pauses_listen">
      {$t("native.ui.talk.pausedListening")}
    </p>
  {/if}

  <p
    id="gs-talk-state"
    class="gs-talk__line"
    data-testid="parent_talk_state"
    data-phase={talk.talk.kind}
    aria-live={mayAnnounce(talk) ? "polite" : "off"}
  >
    {#if hint === "hold"}
      {$t("webScreens.talk.holdHint")}
    {:else if hint === "no_lock"}
      {$t("webScreens.talk.noLock")}
    {:else if stateText}
      {$t(stateText.key, stateText.args)}
    {/if}
  </p>

  <p id="gs-talk-reason" class="gs-talk__line gs-hint" data-testid="parent_talk_unavailable_reason">
    {#if unavailable && !stateText}{$t(unavailable.key, unavailable.args)}{/if}
  </p>

  {#if noise}
    <p class="gs-talk__line gs-hint" data-testid="parent_noise_paused">{$t(noise.key)}</p>
  {/if}

  <!-- A visible hint only: no live region, so nothing is spoken into the open microphone. -->
  {#if talk.talkMicSilent && talk.talk.kind === "talking"}
    <p class="gs-talk__line" data-tone="warn" data-testid="parent_talk_mic_silent">
      {$t("webScreens.talk.micSilent")}
    </p>
  {/if}

  {#if showUnconfirmed}
    <p class="gs-talk__line" data-tone="warn" role="status" data-testid="parent_talk_unconfirmed">
      {$t("webScreens.talk.unconfirmed")}
    </p>
  {:else if talk.talkAudibleUncertain && talk.talk.kind === "idle"}
    <p class="gs-talk__line" data-tone="warn" data-testid="parent_talk_uncertain">
      {$t("webScreens.talk.audibleUncertain")}
    </p>
  {/if}

  {#if talk.afterTalkListenUntilElapsedMs !== null && !busy}
    <Button variant="text" testid="parent_talk_after_listen" onclick={onkeeplisten}>
      {$t("webScreens.talk.afterListen")}
    </Button>
  {/if}
</div>
