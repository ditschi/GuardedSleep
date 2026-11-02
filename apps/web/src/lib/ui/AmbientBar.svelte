<script lang="ts">
  import { useController } from "../contract";
  import { t } from "../i18n/store";
  import { formatDuration } from "./format";
  import Glyph from "./Glyph.svelte";
  import { coreMessageKey } from "./messages";
  import { AMBIENT_KEY, AMBIENT_LOOK, ambientKind, downForMs } from "./parent-view";
  import { everySecond } from "./ticker";

  type Props = {
    /** Shows "What can I do?" in the bar while the connection is in trouble. */
    onhelp?: () => void;
    testid?: string;
  };

  let { onhelp, testid = "parent_ambient_bar" }: Props = $props();

  const { app, parent } = useController();

  const kind = $derived(ambientKind($parent.link));
  const look = $derived(AMBIENT_LOOK[kind]);
  const name = $derived($app.peerName ?? $t("native.ui.common.babyUnitDefault"));
  const text = $derived.by(() => {
    if (kind === "lost_for") {
      return $t(AMBIENT_KEY.lost_for, {
        duration: formatDuration($t, downForMs($parent.link, $everySecond.elapsedMs)),
      });
    }
    if (kind === "needs_pairing") {
      return $t(coreMessageKey($parent.link.failureKey) ?? AMBIENT_KEY.needs_pairing);
    }
    return $t(AMBIENT_KEY[kind], { name });
  });
  const inTrouble = $derived(kind === "cant_reach" || kind === "lost_for");
</script>

<div
  class="gs-status-bar"
  data-tone={look.tone}
  data-kind={kind}
  data-testid={testid}
  role={look.assertive ? "alert" : "status"}
>
  <Glyph name={look.icon} />
  <span class="gs-ambient__text">{text}</span>
  {#if inTrouble && onhelp}
    <button
      type="button"
      class="gs-btn"
      data-variant="text"
      data-testid="parent_help_reconnect"
      onclick={onhelp}
    >
      {$t("native.ui.help.reconnect.open")}
    </button>
  {/if}
</div>
