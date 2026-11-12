<script lang="ts">
  import { t } from "../i18n/store";
  import Glyph from "./Glyph.svelte";

  type Props = {
    open: boolean;
    /** What the first tap shows for a moment: the connection in words, already translated. */
    statusText: string;
    testid?: string;
    /** Baby: "Parent is talking", shown at once (static, no pulse) while a voice plays. */
    talkingText?: string | null;
    /** Baby: "Last talk: ..." on the first tap, with the status. */
    lastTalkText?: string | null;
    /** Baby: the browser suspended the audio; the line and its action turn sound back on. */
    soundOffText?: string | null;
    onsoundon?: () => void;
  };

  let {
    open = $bindable(),
    statusText,
    testid = "night_screen",
    talkingText = null,
    lastTalkText = null,
    soundOffText = null,
    onsoundon,
  }: Props = $props();

  /** How long the status stays up after the first tap, then the screen goes dark again. */
  const STATUS_MS = 6_000;

  let dialog: HTMLDialogElement | undefined = $state();
  let showStatus = $state(false);
  let opener: Element | null = null;

  // A native modal <dialog> gives the focus trap, Escape and inertness of the page behind it, and
  // sits in the top layer above everything else.
  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) {
      opener = document.activeElement;
      showStatus = false;
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  });

  // The status is a first-tap glance only. A UI timer: it decides nothing about the link.
  $effect(() => {
    if (!showStatus) return;
    const id = setTimeout(() => (showStatus = false), STATUS_MS);
    return () => clearTimeout(id);
  });

  function tap(): void {
    if (showStatus) close();
    else showStatus = true;
  }

  function close(): void {
    showStatus = false;
    open = false;
  }

  function handleClose(): void {
    // Escape closes the native dialog; keep `open` in step and hand the focus back.
    showStatus = false;
    open = false;
    if (opener instanceof HTMLElement) opener.focus();
    opener = null;
  }
</script>

<dialog
  bind:this={dialog}
  class="gs-night"
  data-testid={testid}
  data-woken={showStatus || undefined}
  aria-label={$t("webScreens.night.label")}
  onclose={handleClose}
>
  <button type="button" class="gs-night__tap" data-testid="{testid}_tap" onclick={tap}>
    <span class="gs-night__dot" aria-hidden="true"></span>
    {#if showStatus}
      <span class="gs-night__status" data-testid="{testid}_status" role="status">
        <span class="gs-night__line">
          <Glyph name="link" />
          {$t("webScreens.night.monitoring")}
        </span>
        <span>{statusText}</span>
        {#if lastTalkText}<span data-testid="{testid}_last_talk">{lastTalkText}</span>{/if}
        <span class="gs-night__hint">{$t("webScreens.night.leave")}</span>
      </span>
    {:else}
      <span class="gs-sr-only">{$t("webScreens.night.label")}</span>
    {/if}
  </button>
  {#if talkingText}
    <p class="gs-night__talking" data-testid="{testid}_talking" role="status">
      <Glyph name="mic" />
      {talkingText}
    </p>
  {/if}
  {#if soundOffText}
    <button
      type="button"
      class="gs-night__sound"
      data-testid="{testid}_sound_off"
      onclick={() => onsoundon?.()}
    >
      <Glyph name="sound" />
      {soundOffText}
    </button>
  {/if}
</dialog>
