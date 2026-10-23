<script lang="ts">
  import type { Snippet } from "svelte";
  import { t } from "../i18n/store";
  import Glyph from "./Glyph.svelte";

  type Props = {
    /** Shows the back button and runs this on a click. */
    onback?: () => void;
    backTestid?: string;
    /** Left-aligned text beside the back button, e.g. the app name or the device role. */
    title?: string;
    /** Trailing actions, e.g. the settings button. */
    actions?: Snippet;
  };

  let { onback, backTestid, title, actions }: Props = $props();
</script>

<header class="gs-screen-header">
  {#if onback}
    <button
      type="button"
      class="gs-icon-btn"
      data-testid={backTestid}
      aria-label={$t("layout.backAria")}
      onclick={onback}
    >
      <Glyph name="back" />
    </button>
  {/if}
  {#if title}<span class="gs-screen-header__title">{title}</span>{/if}
  <span class="gs-screen-header__spacer"></span>
  {#if actions}{@render actions()}{/if}
</header>
