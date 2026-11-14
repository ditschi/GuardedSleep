<script lang="ts">
  import { useController } from "../contract";
  import { t } from "../i18n/store";
  import { formatDurationWords, spokenDuration } from "./format";
  import Glyph from "./Glyph.svelte";
  import { coreMessageKey } from "./messages";
  import { STATUS_KEY, STATUS_LOOK, STATUS_SUB_KEY, downForMs, statusKindOf } from "./parent-view";
  import { everySecond } from "./ticker";

  /**
   * The parent's status bar (`connection-status` "Parent ambient bar" and "Ambient bar per level"):
   * one component for every parent screen while monitoring. Connected is neutral with a green dot,
   * so a coloured bar always means a change; the other stages use the fixed safety colours. Only the
   * stage words are a live region, the ticking duration beside them never is.
   */
  type Props = { testid?: string };

  let { testid = "status_bar" }: Props = $props();

  const { app, parent } = useController();

  const kind = $derived(statusKindOf($parent.link));
  const look = $derived(STATUS_LOOK[kind]);
  // A connection name is shown as "State · Name"; it is never spliced into a sentence.
  const name = $derived($app.peerName?.trim() || null);
  const headline = $derived.by(() => {
    if (kind === "needs_pairing") {
      return $t(coreMessageKey($parent.link.failureKey) ?? STATUS_KEY.needs_pairing);
    }
    const state = $t(STATUS_KEY[kind]);
    return name === null || kind === "idle"
      ? state
      : $t("native.ui.status.withName", { state, name });
  });
  const subKey = $derived(STATUS_SUB_KEY[kind]);
  const downMs = $derived(downForMs($parent.link, $everySecond.elapsedMs));
</script>

<div
  class="gs-status-bar"
  data-tone={look.tone}
  data-kind={kind}
  data-loud={look.loud || undefined}
  data-testid={testid}
>
  {#if look.tone === "ok"}<span class="gs-status-bar__dot" aria-hidden="true"></span>{/if}
  <Glyph name={look.icon} />
  <div
    class="gs-status-bar__text"
    role={look.assertive ? "alert" : "status"}
    data-testid="status_bar_text"
  >
    <span>{headline}</span>
    {#if subKey}<span class="gs-status-bar__sub">{$t(subKey)}</span>{/if}
  </div>
  {#if look.loud}
    <span class="gs-status-bar__duration" data-testid="status_bar_duration">
      <span class="gs-sr-only">{spokenDuration($t, downMs)}</span>
      <span aria-hidden="true">{formatDurationWords($t, downMs)}</span>
    </span>
  {/if}
</div>
