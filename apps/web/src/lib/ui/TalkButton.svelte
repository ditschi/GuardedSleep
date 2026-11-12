<script lang="ts">
  import type { WebTalkState } from "../contract";
  import { t } from "../i18n/store";
  import Button from "./Button.svelte";
  import Glyph from "./Glyph.svelte";
  import {
    TALK_COUNTDOWN_FROM_MS,
    TALK_HINT_MS,
    TALK_HOLD_MS,
    TALK_LOCK_DISTANCE,
  } from "./talk-constants";
  import { mayAnnounce, noiseLine, stateLine, unavailableLine, type TalkView } from "./talk-view";

  type Props = {
    talk: TalkView;
    /** What the browser adds: no microphone support here. */
    web: WebTalkState;
    /** The monotonic clock of the page (countdowns). */
    nowMs: number;
    /** The hold time passed: start the talk. */
    onpress: () => void;
    /** Released, cancelled or interrupted. */
    onrelease: () => void;
    onkeeplisten: () => void;
  };

  let { talk, web, nowMs, onpress, onrelease, onkeeplisten }: Props = $props();

  let holdTimer: ReturnType<typeof setTimeout> | null = null;
  let hintTimer: ReturnType<typeof setTimeout> | null = null;
  let pointerId: number | null = null;
  let startY = 0;
  // Set once the hold time passed and the talk was asked for: only then does a release count.
  let started = false;
  let hint = $state<"hold" | "no_lock" | null>(null);

  const unavailable = $derived(unavailableLine(talk, web, nowMs));
  const stateText = $derived(stateLine(talk, nowMs));
  const noise = $derived(noiseLine(talk));
  const disabled = $derived(unavailable !== null);
  const busy = $derived(talk.talk.kind === "requesting" || talk.talk.kind === "talking");
  // A line the user must read after the talk; never while the microphone is open.
  const showUnconfirmed = $derived(talk.talkUnconfirmed && talk.talk.kind !== "talking");

  function showHint(kind: "hold" | "no_lock"): void {
    hint = kind;
    if (hintTimer) clearTimeout(hintTimer);
    hintTimer = setTimeout(() => (hint = null), TALK_HINT_MS);
  }

  function begin(): void {
    if (disabled || holdTimer !== null || started) return;
    holdTimer = setTimeout(() => {
      holdTimer = null;
      started = true;
      onpress();
    }, TALK_HOLD_MS);
  }

  function end(tapHint: boolean): void {
    if (holdTimer !== null) {
      clearTimeout(holdTimer);
      holdTimer = null;
      // A short tap is not a talk: say how it works.
      if (tapHint) showHint("hold");
    }
    if (started) {
      started = false;
      onrelease();
    }
    pointerId = null;
  }

  function onpointerdown(event: PointerEvent): void {
    if (disabled || event.button !== 0) return;
    // Keeps the release coming even when the finger slides off the button.
    (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
    pointerId = event.pointerId;
    startY = event.clientY;
    begin();
  }

  function onpointermove(event: PointerEvent): void {
    if (pointerId !== event.pointerId) return;
    if (startY - event.clientY >= TALK_LOCK_DISTANCE) showHint("no_lock");
  }

  function onpointerup(event: PointerEvent): void {
    if (pointerId === event.pointerId) end(true);
  }

  function onpointercancel(event: PointerEvent): void {
    if (pointerId === event.pointerId) end(false);
  }

  // Holding a key is the keyboard's way to hold the button.
  function onkeydown(event: KeyboardEvent): void {
    if ((event.key === " " || event.key === "Enter") && !event.repeat) {
      event.preventDefault();
      begin();
    }
  }

  function onkeyup(event: KeyboardEvent): void {
    if (event.key === " " || event.key === "Enter") end(true);
  }

  // The page leaving the front ends the talk and closes the microphone.
  function onhidden(): void {
    if (document.hidden) end(false);
  }

  // Haptics: a distinct pulse once the microphone is attached, two at 5 s left, one at the end.
  let lastPhase: TalkView["talk"]["kind"] = "idle";
  let countdownBuzzed = false;
  function vibrate(pattern: number | number[]): void {
    if (typeof navigator.vibrate === "function") navigator.vibrate(pattern);
  }
  $effect(() => {
    const phase = talk.talk.kind;
    if (phase === lastPhase) return;
    if (phase === "talking") {
      countdownBuzzed = false;
      vibrate(60);
    } else if (lastPhase === "talking") {
      vibrate([40, 60, 40]);
    }
    lastPhase = phase;
  });
  $effect(() => {
    const state = talk.talk;
    if (state.kind !== "talking" || countdownBuzzed) return;
    if (state.untilElapsedMs - nowMs <= TALK_COUNTDOWN_FROM_MS) {
      countdownBuzzed = true;
      vibrate([80, 80, 80]);
    }
  });
</script>

<svelte:window onblur={() => end(false)} />
<svelte:document onvisibilitychange={onhidden} />

<div class="gs-talk">
  <button
    type="button"
    class="gs-talk__btn"
    data-testid="parent_talk"
    data-phase={talk.talk.kind}
    {disabled}
    aria-pressed={busy}
    aria-describedby="gs-talk-state gs-talk-reason"
    {onpointerdown}
    {onpointermove}
    {onpointerup}
    {onpointercancel}
    {onkeydown}
    {onkeyup}
  >
    <Glyph name="mic" />
    <span>{$t("webScreens.talk.hold")}</span>
  </button>

  <!-- While the microphone is open nothing here is announced (no speech into the open microphone);
       the result is announced after the release. -->
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
