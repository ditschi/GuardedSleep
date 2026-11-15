<script lang="ts">
  import type { WebTalkState } from "../contract";
  import { locale, t } from "../i18n/store";
  import { formatClock } from "./format";
  import Glyph from "./Glyph.svelte";
  import { elapsedToWall } from "./parent-view";
  import {
    TALK_COUNTDOWN_FROM_MS,
    TALK_HINT_MS,
    TALK_HOLD_MS,
    TALK_LOCK_DISTANCE,
  } from "./talk-constants";
  import { stateLine, unavailableLine, type TalkHint, type TalkView } from "./talk-view";

  /**
   * The Talk tile of the dock (`redesign-calm-ui` design section 9). Hold-to-talk behaves as
   * `add-talk-back` built it: pointer hold, Space/Enter hold, release ends, no lock. While the baby
   * device is unavailable for a known time the tile is an outlined status slot, never a disabled
   * button. The lines under the dock are `TalkStatus.svelte`.
   */
  type Props = {
    talk: TalkView;
    /** What the browser adds: no microphone support here. */
    web: WebTalkState;
    /** The monotonic clock of the page (countdowns). */
    nowMs: number;
    /** The wall clock, for "at 02:10". */
    nowWallMs: number;
    /** The hold time passed: start the talk. */
    onpress: () => void;
    /** Released, cancelled or interrupted. */
    onrelease: () => void;
    /** What a short tap or a slide taught the user; shown by the status lines. */
    hint?: TalkHint | null;
  };

  let { talk, web, nowMs, nowWallMs, onpress, onrelease, hint = $bindable(null) }: Props = $props();

  let holdTimer: ReturnType<typeof setTimeout> | null = null;
  let hintTimer: ReturnType<typeof setTimeout> | null = null;
  let pointerId: number | null = null;
  let startY = 0;
  // Set once the hold time passed and the talk was asked for: only then does a release count.
  let started = false;

  const unavailable = $derived(unavailableLine(talk, web, nowMs));
  const stateText = $derived(stateLine(talk, nowMs));
  const disabled = $derived(unavailable !== null);
  const busy = $derived(talk.talk.kind === "requesting" || talk.talk.kind === "talking");
  // Unavailable for a known time: an outlined status slot at full contrast, not a dead button.
  const retryAt = $derived(
    talk.talk.kind === "unavailable" && talk.talk.reason === "cooldown" && !web.micUnsupported
      ? talk.talk.retryAtElapsedMs
      : null,
  );
  const notConnected = $derived(
    talk.talk.kind === "unavailable" && talk.talk.reason === "not_connected",
  );
  const label = $derived(
    notConnected
      ? $t("native.ui.talk.disabled")
      : busy && stateText
        ? $t(stateText.key, stateText.args)
        : $t("webScreens.talk.hold"),
  );

  function showHint(kind: TalkHint): void {
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

{#if retryAt !== null}
  <div
    class="gs-tile"
    data-status
    data-testid="parent_talk"
    data-phase="unavailable"
    aria-describedby="gs-talk-state gs-talk-reason"
  >
    <Glyph name="mic" />
    <span class="gs-tile__text">
      <span class="gs-tile__label">{$t("native.ui.talk.again")}</span>
      <span class="gs-tile__sub">
        {$t("native.ui.talk.againAt", {
          time: formatClock(elapsedToWall(retryAt, nowMs, nowWallMs), $locale),
        })}
      </span>
    </span>
  </div>
{:else}
  <button
    type="button"
    class="gs-tile"
    data-kind="talk"
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
    <span class="gs-tile__text"><span class="gs-tile__label">{label}</span></span>
  </button>
{/if}
