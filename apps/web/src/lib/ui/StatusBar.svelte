<script lang="ts">
  import { useController } from "../contract";
  import { t } from "../i18n/store";
  import { formatDurationWords, spokenDuration } from "./format";
  import Glyph from "./Glyph.svelte";
  import {
    STATUS_LOOK,
    STATUS_SUB_KEY,
    connectionName,
    downForMs,
    statusHeadline,
    statusKindOf,
  } from "./parent-view";
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
  const name = $derived(connectionName($app.peerName));
  const headline = $derived(statusHeadline($t, kind, $parent.link, name));
  const subKey = $derived(STATUS_SUB_KEY[kind]);
  const downMs = $derived(downForMs($parent.link, $everySecond.elapsedMs));
  // The baby device's last reading only counts while connected; unknown shows nothing.
  const battery = $derived($parent.link.phase === "connected" ? $parent.peer.batteryPct : null);
  const charging = $derived($parent.peer.charging === true);
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
    data-testid={`${testid}_text`}
  >
    <span>{headline}</span>
    {#if subKey}<span class="gs-status-bar__sub">{$t(subKey)}</span>{/if}
  </div>
  {#if look.loud}
    <span class="gs-status-bar__duration" data-testid={`${testid}_duration`}>
      <span class="gs-sr-only">{spokenDuration($t, downMs)}</span>
      <span aria-hidden="true">{formatDurationWords($t, downMs)}</span>
    </span>
  {/if}
  {#if battery !== null}
    <span class="gs-status-bar__battery" data-testid={`${testid}_battery`}>
      {#if charging}<Glyph name="bolt" />{/if}
      <span aria-hidden="true">{$t("native.ui.monitor.batteryShort", { percent: battery })}</span>
      <span class="gs-sr-only">
        {$t(charging ? "native.ui.monitor.batteryChargingA11y" : "native.ui.monitor.batteryA11y", {
          percent: battery,
        })}
      </span>
    </span>
  {/if}
</div>
